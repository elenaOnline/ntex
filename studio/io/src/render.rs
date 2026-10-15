//! Rendered previews: a small .tex → tectonic (with
//! `-Z search-path=<templates/tex>` so her theme is found) → PDF → PNG at 2×,
//! rasterised in-process with CoreGraphics (no window, no viewer). Cached by
//! the hash of the source; a newer request makes an older one's result moot.
//!
//! - Snippet jobs: `standalone` (varwidth, 2 pt border) with amsmath, amssymb,
//!   mathtools and XCharter + newtxmath (charter), as her notes. Tab stops arrive as
//!   `\square` (codegen's `preview_latex`); `\square` is drawn in the theme
//!   blue. Document jobs compile as they are.
//! - Cache: `<state>/render-cache/<hash>.png` (+ `<hash>.json`, its size).
//!   The hash covers the source and, for documents, every .sty in
//!   templates/tex (a new block changes the specimen).
//! - A TeX error comes back as the first `!` line of the log
//!   ("! Undefined control sequence.").
//! - A newer job of the same kind kills an older one still compiling (its
//!   receiver gets `Err("superseded")`). Drop kills whatever is running.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// What to render.
#[derive(Clone, Debug, PartialEq)]
pub enum RenderJob {
    /// A snippet's output: `preview_latex` text, in maths (inline or display) or text.
    Snippet { latex: String, math: bool, display: bool },
    /// A full specimen document (blocks::specimen).
    Document { tex: String },
}

/// A finished render: the PNG and its size in points (1×).
#[derive(Clone, Debug, PartialEq)]
pub struct Rendered {
    pub png: PathBuf,
    pub width: f32,
    pub height: f32,
}

/// Pixels per point in the PNG.
pub const SCALE: f64 = 2.0;
/// A compile that takes longer than this is killed.
const TIMEOUT: Duration = Duration::from_secs(30);

pub struct Renderer {
    inner: Arc<Inner>,
}

struct Inner {
    templates_tex: PathBuf,
    cache: PathBuf,
    /// latest generation per kind (0 snippet, 1 document)
    latest: [AtomicU64; 2],
    next: AtomicU64,
    /// running tectonic process groups: generation → (kind, pgid)
    running: Mutex<HashMap<u64, (usize, i32)>>,
}

impl RenderJob {
    fn kind(&self) -> usize {
        match self {
            RenderJob::Snippet { .. } => 0,
            RenderJob::Document { .. } => 1,
        }
    }
}

/// The .tex for a job.
pub fn source(job: &RenderJob) -> String {
    match job {
        RenderJob::Document { tex } => tex.clone(),
        RenderJob::Snippet { latex, math, display } => {
            let body = if !*math {
                latex.clone()
            } else if *display {
                format!("\\[\n{latex}\n\\]")
            } else {
                format!("$\\displaystyle {latex}$")
            };
            format!(
                "\\documentclass[varwidth,border=2pt]{{standalone}}\n\
                 \\usepackage{{amsmath,amssymb,mathtools}}\n\
                 \\usepackage{{XCharter}}\\usepackage[charter,vvarbb]{{newtxmath}}\n\
                 \\usepackage{{xcolor}}\n\
                 \\definecolor{{ntexBlue}}{{HTML}}{{1E47B8}}\n\
                 \\let\\ntexSquare\\square\n\
                 \\renewcommand{{\\square}}{{\\mathord{{\\textcolor{{ntexBlue}}{{\\ntexSquare}}}}}}\n\
                 \\begin{{document}}\n{body}\n\\end{{document}}\n"
            )
        }
    }
}

/// FNV-1a, 64 bit: stable across runs and Rust versions (the cache key).
pub fn hash(parts: &[&[u8]]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for p in parts {
        for b in p.iter().chain(&[0xffu8]) {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    }
    h
}

impl Renderer {
    pub fn new(paths: &ntex_studio_core::model::Paths) -> Renderer {
        Renderer::with_dirs(&paths.templates_tex, &paths.state.join("render-cache"))
    }

    /// Like `new`, with the folders spelled out (tests).
    pub fn with_dirs(templates_tex: &Path, cache: &Path) -> Renderer {
        Renderer {
            inner: Arc::new(Inner {
                templates_tex: templates_tex.to_path_buf(),
                cache: cache.to_path_buf(),
                latest: [AtomicU64::new(0), AtomicU64::new(0)],
                next: AtomicU64::new(1),
                running: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Start a render in the background; the receiver gets Ok(Rendered) or
    /// Err(first line of the TeX error).
    pub fn render(&self, job: RenderJob) -> mpsc::Receiver<Result<Rendered, String>> {
        let (tx, rx) = mpsc::channel();
        let inner = self.inner.clone();
        let generation = inner.next.fetch_add(1, Ordering::SeqCst);
        let kind = job.kind();
        inner.latest[kind].store(generation, Ordering::SeqCst);
        // an older compile of the same kind is moot: stop it
        {
            let running = inner.running.lock().unwrap();
            for (g, (k, pgid)) in running.iter() {
                if *k == kind && *g < generation {
                    kill_group(*pgid);
                }
            }
        }
        std::thread::Builder::new()
            .name("ntex-render".into())
            .spawn(move || {
                let res = inner.run(&job, generation);
                let _ = tx.send(res);
            })
            .expect("spawn render thread");
        rx
    }

    /// The same, blocking (tests, measurements).
    pub fn render_blocking(&self, job: RenderJob) -> Result<Rendered, String> {
        self.render(job).recv().unwrap_or_else(|_| Err("the render thread stopped".into()))
    }

    /// Where a job's PNG is (or will be) cached.
    pub fn cache_path(&self, job: &RenderJob) -> PathBuf {
        self.inner.cache.join(format!("{:016x}.png", self.inner.key(job)))
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        if let Ok(running) = self.inner.running.lock() {
            for (_, pgid) in running.values() {
                kill_group(*pgid);
            }
        }
    }
}

impl Inner {
    fn key(&self, job: &RenderJob) -> u64 {
        let src = source(job);
        let mut parts: Vec<Vec<u8>> = vec![b"ntex-render-v1".to_vec(), src.into_bytes()];
        if let RenderJob::Document { .. } = job {
            // the theme (and the blocks) are part of a document's look
            if let Ok(rd) = std::fs::read_dir(&self.templates_tex) {
                let mut names: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "sty")).collect();
                names.sort();
                for p in names {
                    parts.push(p.file_name().unwrap().to_string_lossy().as_bytes().to_vec());
                    parts.push(std::fs::read(&p).unwrap_or_default());
                }
            }
        }
        let refs: Vec<&[u8]> = parts.iter().map(|v| v.as_slice()).collect();
        hash(&refs)
    }

    fn run(&self, job: &RenderJob, generation: u64) -> Result<Rendered, String> {
        let key = self.key(job);
        let png = self.cache.join(format!("{key:016x}.png"));
        let meta = self.cache.join(format!("{key:016x}.json"));
        if let (true, Ok(m)) = (png.exists(), std::fs::read_to_string(&meta)) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&m) {
                if let (Some(w), Some(h)) = (v["width"].as_f64(), v["height"].as_f64()) {
                    return Ok(Rendered { png, width: w as f32, height: h as f32 });
                }
            }
        }
        std::fs::create_dir_all(&self.cache).map_err(|e| format!("can't make {}: {e}", self.cache.display()))?;
        let work = self.cache.join(format!("work-{key:016x}-{}-{generation}", std::process::id()));
        let _ = std::fs::remove_dir_all(&work);
        std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
        let res = self.compile_and_raster(job, generation, &work, &png, &meta);
        let _ = std::fs::remove_dir_all(&work);
        res
    }

    fn compile_and_raster(&self, job: &RenderJob, generation: u64, work: &Path, png: &Path, meta: &Path) -> Result<Rendered, String> {
        let tex = work.join("preview.tex");
        std::fs::write(&tex, source(job)).map_err(|e| e.to_string())?;
        let pdf = compile(&tex, work, &self.templates_tex, |pgid| {
            self.running.lock().unwrap().insert(generation, (job.kind(), pgid));
        });
        self.running.lock().unwrap().remove(&generation);
        if self.latest[job.kind()].load(Ordering::SeqCst) != generation && pdf.is_err() {
            return Err("superseded".into());
        }
        let pdf = pdf?;
        let tmp_png = png.with_extension("png.part");
        let (w, h) = pdf_to_png(&pdf, &tmp_png, SCALE)?;
        std::fs::rename(&tmp_png, png).map_err(|e| e.to_string())?;
        let _ = std::fs::write(meta, serde_json::json!({ "width": w, "height": h }).to_string());
        Ok(Rendered { png: png.to_path_buf(), width: w as f32, height: h as f32 })
    }
}

fn kill_group(pgid: i32) {
    if pgid > 0 {
        unsafe {
            libc::killpg(pgid, libc::SIGKILL);
        }
    }
}

/// tectonic `tex` → PDF in `outdir`. `started` gets the process group id.
/// Err: the first `!` line of the log (or tectonic's own error).
pub fn compile(tex: &Path, outdir: &Path, templates_tex: &Path, started: impl FnOnce(i32)) -> Result<PathBuf, String> {
    use std::os::unix::process::CommandExt;
    let mut cmd = Command::new(tectonic_path());
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
    let pgid = child.id() as i32;
    started(pgid);
    // stderr is drained on its own thread so a chatty run never blocks
    let mut stderr = child.stderr.take();
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(e) = stderr.as_mut() {
            use std::io::Read;
            let _ = e.read_to_string(&mut s);
        }
        s
    });
    let deadline = std::time::Instant::now() + TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break Some(st),
            Ok(None) if std::time::Instant::now() > deadline => {
                kill_group(pgid);
                let _ = child.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(5)),
            Err(_) => break None,
        }
    };
    let stderr = reader.join().unwrap_or_default();
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
            if st.code().is_none() {
                return Err("superseded".into());
            }
            Err(format!("tectonic failed ({st})"))
        }
        None => Err("tectonic took longer than 30 s".into()),
    }
}

fn tectonic_path() -> PathBuf {
    for p in ["/opt/homebrew/bin/tectonic", "/usr/local/bin/tectonic"] {
        if Path::new(p).exists() {
            return PathBuf::from(p);
        }
    }
    PathBuf::from("tectonic")
}

/// Rasterise every page of `pdf` (stacked top to bottom) into a PNG at
/// `scale` pixels per point, on white. In-process CoreGraphics: no window,
/// no viewer. Returns the size in points.
pub fn pdf_to_png(pdf: &Path, png: &Path, scale: f64) -> Result<(f64, f64), String> {
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};
    use objc2_core_graphics::{CGBitmapContextCreate, CGColorSpace, CGContext, CGDataProvider, CGPDFBox, CGPDFDocument, CGPDFPage};
    use std::os::unix::ffi::OsStrExt;

    let cpath = std::ffi::CString::new(pdf.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
    let provider = unsafe { CGDataProvider::with_filename(cpath.as_ptr()) }.ok_or_else(|| format!("can't read {}", pdf.display()))?;
    let doc = CGPDFDocument::with_provider(Some(&provider)).ok_or_else(|| format!("{} isn't a PDF", pdf.display()))?;
    let n = CGPDFDocument::number_of_pages(Some(&doc)).min(20);
    if n == 0 {
        return Err("the PDF has no pages".into());
    }
    let mut pages = Vec::new();
    let (mut wpt, mut hpt) = (0.0f64, 0.0f64);
    for i in 1..=n {
        let page = CGPDFDocument::page(Some(&doc), i).ok_or("a PDF page is missing")?;
        let r = CGPDFPage::box_rect(Some(&page), CGPDFBox::CropBox);
        wpt = wpt.max(r.size.width);
        hpt += r.size.height;
        pages.push((page, r));
    }
    let w = (wpt * scale).ceil().max(1.0) as usize;
    let h = (hpt * scale).ceil().max(1.0) as usize;
    if w * h > 40_000_000 {
        return Err("the page is too large to preview".into());
    }
    let mut buf = vec![0u8; w * h * 4];
    let space = CGColorSpace::with_name(Some(unsafe { objc2_core_graphics::kCGColorSpaceSRGB })).or_else(CGColorSpace::new_device_rgb).ok_or("no colour space")?;
    // RGBA, premultiplied, big-endian bytes: R G B A in memory
    const PREMULTIPLIED_LAST: u32 = 1;
    const ORDER_32_BIG: u32 = 4 << 12;
    let ctx = unsafe { CGBitmapContextCreate(buf.as_mut_ptr() as *mut std::ffi::c_void, w, h, 8, w * 4, Some(&space), PREMULTIPLIED_LAST | ORDER_32_BIG) }
        .ok_or("CoreGraphics couldn't make a bitmap")?;
    let c = Some(&*ctx);
    CGContext::set_rgb_fill_color(c, 1.0, 1.0, 1.0, 1.0);
    CGContext::fill_rect(c, CGRect { origin: CGPoint { x: 0.0, y: 0.0 }, size: CGSize { width: w as f64, height: h as f64 } });
    let mut top = 0.0f64; // points from the top of the stack
    for (page, r) in &pages {
        CGContext::save_g_state(c);
        // CG's origin is bottom left: this page's bottom edge sits at hpt - top - height
        CGContext::scale_ctm(c, scale, scale);
        CGContext::translate_ctm(c, -r.origin.x, hpt - top - r.size.height - r.origin.y);
        CGContext::draw_pdf_page(c, Some(page));
        CGContext::restore_g_state(c);
        top += r.size.height;
    }
    drop(ctx);
    // white everywhere underneath, so alpha is 255: write RGB
    let mut rgb = Vec::with_capacity(w * h * 3);
    for px in buf.chunks_exact(4) {
        rgb.extend_from_slice(&px[..3]);
    }
    let file = std::fs::File::create(png).map_err(|e| format!("can't write {}: {e}", png.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
    let mut writer = enc.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(&rgb).map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())?;
    Ok((wpt, hpt))
}
