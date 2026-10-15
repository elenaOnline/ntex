//! Shared test helpers: temp copies, never her real files.
#![allow(dead_code)]
use ntex_studio_core::model::Paths;
use std::path::{Path, PathBuf};

pub fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap())
}

pub fn real_config() -> PathBuf {
    home().join(".config/texnvim")
}

/// A fresh folder under $TMPDIR, removed on drop.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(tag: &str) -> TempDir {
        let base = std::env::var_os("TMPDIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"));
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let p = base.join(format!("ntex-io-test-{tag}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&p).unwrap();
        TempDir(p)
    }
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let p = e.path();
        let dest = to.join(e.file_name());
        if p.is_dir() {
            copy_dir(&p, &dest);
        } else {
            // read + write, not fs::copy: APFS clones show up as events on
            // the source, and her running ntex sessions would reload
            std::fs::write(&dest, std::fs::read(&p).unwrap()).unwrap();
        }
    }
}

/// Paths over a temp copy of the snippets folder and a temp state dir. The
/// config itself (lua/, templates/) is only read.
pub fn temp_paths(t: &TempDir) -> Paths {
    let snippets = t.path().join("snippets");
    copy_dir(&real_config().join("snippets"), &snippets);
    // a draft left over from a crashed run must not be in the copy
    let _ = std::fs::remove_file(snippets.join("tex/zz_studio_try.lua"));
    let state = t.path().join("state");
    std::fs::create_dir_all(&state).unwrap();
    Paths { config: real_config(), snippets, templates_tex: real_config().join("templates/tex"), state }
}

/// Processes in a process group (pgrep -g).
pub fn group_members(pgid: u32) -> Vec<(u32, String)> {
    let out = std::process::Command::new("pgrep").arg("-g").arg(pgid.to_string()).output().unwrap();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.trim().parse::<u32>().ok())
        .map(|pid| {
            let o = std::process::Command::new("ps").args(["-o", "command=", "-p", &pid.to_string()]).output().unwrap();
            (pid, String::from_utf8_lossy(&o.stdout).trim().to_string())
        })
        .collect()
}

pub fn alive(pid: u32) -> bool {
    unsafe { libc_kill(pid as i32, 0) == 0 }
}

unsafe extern "C" {
    #[link_name = "kill"]
    fn libc_kill(pid: i32, sig: i32) -> i32;
}
