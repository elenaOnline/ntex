//! Building PDFs with tectonic (search path = templates/tex), atomically.
//!
//! The document is compiled in a fresh temp dir; the PDF is then copied next
//! to the target as `.<name>.pdf.part` and renamed over it, so a reader never
//! sees half a file and a failed build never replaces a good PDF.
use crate::model::*;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub struct Built {
    pub pdf: PathBuf,
    pub warnings: Vec<String>,
}

/// tectonic gets this long before it is killed.
const TIMEOUT: Duration = Duration::from_secs(90);

/// Parse + layout + compile `fig_path` into the PDF next to it (same stem).
/// Err = the first TeX error line (the old PDF is kept).
pub fn build(fig_path: &Path, templates_tex: &Path) -> Result<Built, String> {
    build_cancellable(fig_path, templates_tex, &AtomicBool::new(false))
}

/// `build`, stopped when `cancel` turns true: tectonic (its whole process
/// group) is killed and reaped within a few ms, the old PDF is kept, and the
/// result is Err(CANCELLED). The figure window sets it when the studio quits,
/// so no tectonic outlives the studio.
pub fn build_cancellable(fig_path: &Path, templates_tex: &Path, cancel: &AtomicBool) -> Result<Built, String> {
    let src = std::fs::read_to_string(fig_path).map_err(|e| format!("can't read {}: {e}", fig_path.display()))?;
    build_source_cancellable(&src, &fig_path.with_extension("pdf"), templates_tex, cancel)
}

/// What a cancelled build returns.
pub const CANCELLED: &str = "cancelled";

/// Build from source text into `out_pdf` (the window's live preview).
pub fn build_source(src: &str, out_pdf: &Path, templates_tex: &Path) -> Result<Built, String> {
    build_source_cancellable(src, out_pdf, templates_tex, &AtomicBool::new(false))
}

/// `build_source`, stopped when `cancel` turns true (see `build_cancellable`).
pub fn build_source_cancellable(src: &str, out_pdf: &Path, templates_tex: &Path, cancel: &AtomicBool) -> Result<Built, String> {
    let (fig, diags) = crate::parse::parse(src);
    let warnings: Vec<String> = diags.iter().map(diagnostic_line).collect();
    let mut built = build_figure_cancellable(&fig, out_pdf, templates_tex, cancel)?;
    built.warnings.splice(0..0, warnings);
    Ok(built)
}

/// "line 4: … " as the CLI and the window show it.
pub fn diagnostic_line(d: &Diagnostic) -> String {
    if d.message.starts_with("line ") || d.span.line == 0 { d.message.clone() } else { format!("line {}: {}", d.span.line, d.message) }
}

/// Problems only layout can see, as diagnostics: a Cayley graph whose group
/// can't be made (unknown name, too big, infinite). The figure still builds,
/// without that graph; nothing about it is typeset (the log says why).
pub fn figure_diagnostics(fig: &Figure) -> Vec<Diagnostic> {
    let mut out = vec![];
    for it in &fig.items {
        if let Item::Cayley(c) = it {
            if let Err(e) = crate::cayley::group(&c.group, &c.gens) {
                out.push(Diagnostic { severity: Severity::Warning, span: c.span, message: format!("line {}: {e}", c.span.line) });
            }
        }
    }
    out
}

/// Is there anything to draw?
pub fn is_empty(fig: &Figure) -> bool {
    !fig.items.iter().any(|it| !matches!(it, Item::Comment { .. } | Item::Grid { .. }))
}

/// The starting file for a new figure (figures/template.fig): two comment
/// lines, what the file is and where the reference is (⌘/ in the window,
/// `reference.md`). `NAME` stands for the figure's name; `template_for`
/// fills it in. Used by `ntex-fig new`, Space g (FIG-NVIM) and the window.
pub const TEMPLATE: &str = include_str!("../template.fig");

/// The shorthand reference the window shows on ⌘/ (the old cheat sheet).
pub const REFERENCE: &str = include_str!("../reference.md");

/// TEMPLATE for the figure `name` (`fig-1`, or `sub/deep` for
/// figures/sub/deep.fig).
pub fn template_for(name: &str) -> String {
    TEMPLATE.replace("NAME", name)
}

/// A .fig path's name as the template says it: the part after the last
/// `figures/` folder, without `.fig` (else just the file's stem).
pub fn figure_name(path: &Path) -> String {
    let parts: Vec<String> = path.with_extension("").components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    match parts.iter().rposition(|c| c == "figures") {
        Some(k) if k + 1 < parts.len() => parts[k + 1..].join("/"),
        _ => path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
    }
}

/// Write the template (`template_for` this file's name) to `path` unless the
/// file exists. Ok(true) = written.
pub fn new_figure(path: &Path) -> Result<bool, String> {
    if path.exists() {
        return Ok(false);
    }
    if let Some(dir) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("can't make {}: {e}", dir.display()))?;
    }
    // create_new: never clobber a file that appeared in the meantime
    use std::io::Write;
    match std::fs::OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut f) => f.write_all(template_for(&figure_name(path)).as_bytes()).map(|_| true).map_err(|e| format!("can't write {}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(e) => Err(format!("can't write {}: {e}", path.display())),
    }
}

/// The note an empty figure's build returns (with Ok).
pub const EMPTY_NOTE: &str = "note: the figure is empty; wrote a placeholder PDF";

/// The placeholder for an empty figure: a dashed slate rounded box (about
/// 4 × 1.5 cm) saying "empty figure — NAME", so the `\includegraphics`
/// Space g just inserted still compiles.
pub fn placeholder_document(name: &str) -> String {
    let safe: String = name.chars().filter(|c| !"{}\\%#$&^~".contains(*c)).collect();
    format!(
        "% Ntex Figures — an empty figure (placeholder).\n\\documentclass[border=3pt]{{standalone}}\n\\usepackage{{ntex-figure}}\n\\begin{{document}}\n\\begin{{tikzpicture}}\n  \\node[draw=ntexFigGrey, dash pattern=on 2.4pt off 1.8pt, line width=0.5pt, rounded corners=4pt, minimum width=4cm, minimum height=1.5cm, text=ntexFigGrey, font=\\small] {{empty figure --- \\detokenize{{{safe}}}}};\n\\end{{tikzpicture}}\n\\end{{document}}\n"
    )
}

/// Layout + compile an already-parsed Figure into `out_pdf`. An empty figure
/// gets the placeholder PDF (Ok, with EMPTY_NOTE as its warning).
pub fn build_figure(fig: &Figure, out_pdf: &Path, templates_tex: &Path) -> Result<Built, String> {
    build_figure_cancellable(fig, out_pdf, templates_tex, &AtomicBool::new(false))
}

/// `build_figure`, stopped when `cancel` turns true (see `build_cancellable`).
pub fn build_figure_cancellable(fig: &Figure, out_pdf: &Path, templates_tex: &Path, cancel: &AtomicBool) -> Result<Built, String> {
    if is_empty(fig) {
        let name = out_pdf.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "figure".into());
        compile_tex_cancellable(&placeholder_document(&name), out_pdf, templates_tex, cancel)?;
        return Ok(Built { pdf: out_pdf.to_path_buf(), warnings: vec![EMPTY_NOTE.into()] });
    }
    let scene = crate::layout::layout(fig);
    let tex = crate::tikz::document(fig, &scene);
    compile_tex_cancellable(&tex, out_pdf, templates_tex, cancel)?;
    Ok(Built { pdf: out_pdf.to_path_buf(), warnings: figure_diagnostics(fig).iter().map(diagnostic_line).collect() })
}

static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Compile a complete .tex source and atomically put the PDF at `out_pdf`.
pub fn compile_tex_to(tex: &str, out_pdf: &Path, templates_tex: &Path) -> Result<(), String> {
    compile_tex_cancellable(tex, out_pdf, templates_tex, &AtomicBool::new(false))
}

/// `compile_tex_to`, stopped when `cancel` turns true (see `build_cancellable`).
pub fn compile_tex_cancellable(tex: &str, out_pdf: &Path, templates_tex: &Path, cancel: &AtomicBool) -> Result<(), String> {
    compile_with(&tectonic_path(), tex, out_pdf, templates_tex, cancel)
}

fn compile_with(program: &Path, tex: &str, out_pdf: &Path, templates_tex: &Path, cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::SeqCst) {
        return Err(CANCELLED.into());
    }
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let work = std::env::temp_dir().join(format!("ntex-fig-{}-{n}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| format!("can't make a temp dir: {e}"))?;
    let res = (|| {
        let texp = work.join("figure.tex");
        std::fs::write(&texp, tex).map_err(|e| e.to_string())?;
        let pdf = run_tectonic(program, &texp, &work, templates_tex, cancel)?;
        let dir = out_pdf.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
        std::fs::create_dir_all(dir).map_err(|e| format!("can't make {}: {e}", dir.display()))?;
        let name = out_pdf.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "figure.pdf".into());
        let part = dir.join(format!(".{name}.part-{}", std::process::id()));
        std::fs::copy(&pdf, &part).map_err(|e| format!("can't write {}: {e}", part.display()))?;
        std::fs::rename(&part, out_pdf).map_err(|e| {
            let _ = std::fs::remove_file(&part);
            format!("can't write {}: {e}", out_pdf.display())
        })?;
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&work);
    res
}

fn tectonic_path() -> PathBuf {
    if let Some(p) = std::env::var_os("NTEX_TECTONIC") {
        return PathBuf::from(p);
    }
    for p in ["/opt/homebrew/bin/tectonic", "/usr/local/bin/tectonic"] {
        if Path::new(p).exists() {
            return PathBuf::from(p);
        }
    }
    PathBuf::from("tectonic")
}

unsafe extern "C" {
    fn killpg(pgrp: i32, sig: i32) -> i32;
}

/// Kill tectonic's process group (it was started as its own group leader),
/// then the child itself, and reap it.
fn kill_and_reap(child: &mut std::process::Child) {
    // SAFETY: signals the process group we started (process_group(0))
    unsafe {
        killpg(child.id() as i32, 9);
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// tectonic → PDF in `outdir`. Err: the first `!` line of the log.
fn run_tectonic(program: &Path, tex: &Path, outdir: &Path, templates_tex: &Path, cancel: &AtomicBool) -> Result<PathBuf, String> {
    use std::os::unix::process::CommandExt;
    let mut cmd = Command::new(program);
    cmd.arg("--chatter")
        .arg("minimal")
        .arg("--keep-logs")
        .arg("--outdir")
        .arg(outdir)
        .arg("-Z")
        .arg(format!("search-path={}", templates_tex.display()))
        .arg(tex)
        .current_dir(outdir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .process_group(0);
    let mut child = cmd.spawn().map_err(|e| format!("tectonic didn't start: {e}"))?;
    let mut stderr = child.stderr.take();
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(e) = stderr.as_mut() {
            use std::io::Read;
            let _ = e.read_to_string(&mut s);
        }
        s
    });
    let deadline = Instant::now() + TIMEOUT;
    let mut cancelled = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break Some(st),
            Ok(None) if cancel.load(Ordering::SeqCst) => {
                kill_and_reap(&mut child);
                cancelled = true;
                break None;
            }
            Ok(None) if Instant::now() > deadline => {
                kill_and_reap(&mut child);
                break None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(5)),
            Err(_) => {
                kill_and_reap(&mut child);
                break None;
            }
        }
    };
    let stderr = reader.join().unwrap_or_default();
    if cancelled {
        return Err(CANCELLED.into());
    }
    let stem = tex.file_stem().unwrap().to_string_lossy().to_string();
    let pdf = outdir.join(format!("{stem}.pdf"));
    match status {
        Some(st) if st.success() && pdf.exists() => Ok(pdf),
        Some(st) => {
            let log = std::fs::read_to_string(outdir.join(format!("{stem}.log"))).unwrap_or_default();
            if let Some(l) = log.lines().find(|l| l.starts_with('!')) {
                return Err(l.trim_end().to_string());
            }
            if let Some(l) = stderr.lines().find(|l| l.starts_with("error:")) {
                return Err(l.trim_start_matches("error:").trim().to_string());
            }
            Err(format!("tectonic failed ({st})"))
        }
        None => Err(format!("tectonic took longer than {} s", TIMEOUT.as_secs())),
    }
}

/// ~/.config/texnvim/templates/tex (or $NTEX_STUDIO_CONFIG/templates/tex).
pub fn templates_dir() -> PathBuf {
    if let Some(c) = std::env::var_os("NTEX_STUDIO_CONFIG").filter(|c| !c.is_empty()) {
        return PathBuf::from(c).join("templates").join("tex");
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    home.join(".config").join("texnvim").join("templates").join("tex")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    unsafe extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }

    /// A stand-in tectonic that never finishes: a shell with a child of its
    /// own (sleep), so a kill of the shell alone would leave an orphan.
    fn slow_tectonic(dir: &Path) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let p = dir.join("slow-tectonic");
        std::fs::write(&p, "#!/bin/sh\necho $$ > \"$(dirname \"$0\")/pgid\"\nsleep 30 &\nwait\n").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    #[test]
    fn a_cancelled_build_kills_tectonic_and_keeps_the_old_pdf() {
        let dir = std::env::temp_dir().join(format!("ntex-fig-cancel-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let program = slow_tectonic(&dir);
        let out = dir.join("x.pdf");
        std::fs::write(&out, b"old pdf").unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let c2 = cancel.clone();
        let (p2, o2, d2) = (program.clone(), out.clone(), dir.clone());
        let t = std::thread::spawn(move || compile_with(&p2, "\\relax", &o2, &d2, &c2));
        // wait until the stand-in is running (it writes its pid)
        let pidf = dir.join("pgid");
        let start = Instant::now();
        while !pidf.exists() && start.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(10));
        }
        std::thread::sleep(Duration::from_millis(50));
        let pgid: i32 = std::fs::read_to_string(&pidf).unwrap().trim().parse().unwrap();
        let asked = Instant::now();
        cancel.store(true, Ordering::SeqCst);
        let r = t.join().unwrap();
        assert_eq!(r, Err(CANCELLED.to_string()));
        assert!(asked.elapsed() < Duration::from_secs(2), "cancel took {:?}", asked.elapsed());
        // nothing of the group is left (sleep included)
        std::thread::sleep(Duration::from_millis(50));
        // SAFETY: signal 0 only checks whether the group exists
        assert_ne!(unsafe { kill(-pgid, 0) }, 0, "tectonic's process group outlived the cancel");
        assert_eq!(std::fs::read(&out).unwrap(), b"old pdf", "a cancelled build keeps the old PDF");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cancelled_before_it_starts_runs_nothing() {
        let dir = std::env::temp_dir().join(format!("ntex-fig-cancel0-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let program = slow_tectonic(&dir);
        let r = compile_with(&program, "\\relax", &dir.join("y.pdf"), &dir, &AtomicBool::new(true));
        assert_eq!(r, Err(CANCELLED.to_string()));
        assert!(!dir.join("pgid").exists(), "nothing was started");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
