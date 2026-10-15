//! One studio at a time. `ntex-studio` first tries the socket: if a studio is
//! running, it sends the OpenRequest and exits; otherwise it becomes the
//! studio and listens.
//!
//! Wire format on `Paths::socket()`: the client writes one line of JSON (an
//! OpenRequest), the primary answers "ok\n". A socket file nobody answers on
//! (a studio that crashed) is removed and taken over.
use ntex_studio_core::model::*;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub enum Instance {
    /// We are the studio; requests from later invocations arrive here.
    Primary(std::sync::mpsc::Receiver<OpenRequest>),
    /// Another studio took the request; exit.
    Forwarded,
}

fn forward(sock: &Path, req: &OpenRequest) -> std::io::Result<()> {
    let mut s = UnixStream::connect(sock)?;
    s.set_read_timeout(Some(Duration::from_secs(2)))?;
    s.set_write_timeout(Some(Duration::from_secs(2)))?;
    let line = serde_json::to_string(req).map_err(std::io::Error::other)? + "\n";
    s.write_all(line.as_bytes())?;
    s.flush()?;
    let mut ack = String::new();
    BufReader::new(&s).read_line(&mut ack)?;
    if ack.trim() == "ok" {
        Ok(())
    } else {
        Err(std::io::Error::new(std::io::ErrorKind::ConnectionAborted, "the running studio didn't answer"))
    }
}

fn serve(listener: UnixListener, sock: PathBuf, tx: std::sync::mpsc::Sender<OpenRequest>) {
    for conn in listener.incoming() {
        let Ok(mut conn) = conn else { continue };
        let _ = conn.set_read_timeout(Some(Duration::from_secs(2)));
        let mut line = String::new();
        if BufReader::new(&conn).read_line(&mut line).is_err() {
            continue;
        }
        let req: OpenRequest = serde_json::from_str(line.trim()).unwrap_or_default();
        if tx.send(req).is_err() {
            // the studio is gone (receiver dropped): stop listening
            let _ = conn.write_all(b"gone\n");
            let _ = std::fs::remove_file(&sock);
            return;
        }
        let _ = conn.write_all(b"ok\n");
    }
}

/// Where the socket really is: `Paths::socket()`, unless that path is too
/// long for a Unix socket (104 bytes on macOS; a deep temp state folder in
/// tests), then `/tmp/ntex-studio-<hash of it>.sock`, the same for everyone
/// using that state folder.
pub fn socket_path(paths: &Paths) -> PathBuf {
    let sock = paths.socket();
    if sock.as_os_str().len() < 100 {
        return sock;
    }
    use std::os::unix::ffi::OsStrExt;
    let h = crate::render::hash(&[sock.as_os_str().as_bytes()]);
    PathBuf::from(format!("/tmp/ntex-studio-{h:016x}.sock"))
}

pub fn claim(paths: &Paths, first: &OpenRequest) -> std::io::Result<Instance> {
    let sock = socket_path(paths);
    if let Some(dir) = paths.socket().parent() {
        std::fs::create_dir_all(dir)?;
    }
    for _ in 0..20 {
        if sock.exists() {
            match forward(&sock, first) {
                Ok(()) => return Ok(Instance::Forwarded),
                Err(_) => {
                    // nobody home (or a studio that hung): take the socket over
                    let _ = std::fs::remove_file(&sock);
                }
            }
        }
        match UnixListener::bind(&sock) {
            Ok(listener) => {
                let (tx, rx) = std::sync::mpsc::channel();
                let s2 = sock.clone();
                std::thread::Builder::new().name("ntex-studio-instance".into()).spawn(move || serve(listener, s2, tx))?;
                return Ok(Instance::Primary(rx));
            }
            // another studio bound it a moment ago: forward to it
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                std::thread::sleep(Duration::from_millis(25));
                continue;
            }
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::other(format!("couldn't claim {}", sock.display())))
}

fn under_tmpdir(p: &Path) -> bool {
    let Ok(file) = p.canonicalize() else { return false };
    let mut roots: Vec<PathBuf> = vec![std::env::temp_dir()];
    if let Some(t) = std::env::var_os("TMPDIR") {
        roots.push(PathBuf::from(t));
    }
    roots.iter().filter_map(|r| r.canonicalize().ok()).any(|r| file.starts_with(&r) && file != r)
}

/// Parse the command line: `ntex-studio [--page snippet|block|inbox]
/// [--output-file PATH] [--context math|text] [--ft tex|markdown]`.
///
/// `--output-file`: the file is read into `output`, then deleted, but only
/// if it lives under $TMPDIR (ntex writes the selection there); anything
/// else is left alone. `--opt=value` works too; unknown arguments are ignored.
pub fn parse_args(args: &[String]) -> OpenRequest {
    let mut req = OpenRequest::default();
    let mut i = 0;
    // args[0] may be the program name
    if args.first().is_some_and(|a| !a.starts_with("--")) {
        i = 1;
    }
    while i < args.len() {
        let a = &args[i];
        let (flag, inline) = match a.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f.to_string(), Some(v.to_string())),
            _ => (a.clone(), None),
        };
        let takes = matches!(flag.as_str(), "--page" | "--output-file" | "--context" | "--ft");
        let value = if !takes {
            None
        } else if inline.is_some() {
            inline
        } else {
            i += 1;
            args.get(i).cloned()
        };
        match (flag.as_str(), value) {
            ("--page", Some(v)) => {
                let v = v.to_lowercase();
                if matches!(v.as_str(), "snippet" | "block" | "inbox") {
                    req.page = Some(v);
                }
            }
            ("--output-file", Some(v)) => {
                let p = PathBuf::from(&v);
                if let Ok(text) = std::fs::read_to_string(&p) {
                    req.output = Some(text);
                    if under_tmpdir(&p) {
                        let _ = std::fs::remove_file(&p);
                    }
                }
            }
            ("--context", Some(v)) => {
                if matches!(v.as_str(), "math" | "text") {
                    req.context = Some(v);
                }
            }
            ("--ft", Some(v)) => {
                let v = if v == "md" { "markdown".to_string() } else { v };
                if matches!(v.as_str(), "tex" | "markdown") {
                    req.ft = Some(v);
                }
            }
            _ => {}
        }
        i += 1;
    }
    req
}
