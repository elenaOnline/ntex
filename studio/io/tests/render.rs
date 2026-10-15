//! tectonic → PDF → PNG (CoreGraphics, in-process). Renders into a temp
//! cache; never opens the PDF or the PNG in anything.
mod common;
use common::*;
use ntex_studio_io::render::{RenderJob, Renderer, pdf_to_png, source};
use std::time::Instant;

struct Img {
    w: u32,
    h: u32,
    rgb: Vec<u8>,
}

fn load(p: &std::path::Path) -> Img {
    let dec = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(p).unwrap()));
    let mut r = dec.read_info().unwrap();
    let mut buf = vec![0; r.output_buffer_size().unwrap()];
    let info = r.next_frame(&mut buf).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgb);
    buf.truncate(info.buffer_size());
    Img { w: info.width, h: info.height, rgb: buf }
}

impl Img {
    fn ink(&self) -> usize {
        self.rgb.chunks_exact(3).filter(|p| p.iter().any(|c| *c < 200)).count()
    }
    /// pixels close to a colour (each channel within `tol`)
    fn near(&self, c: (u8, u8, u8), tol: i32) -> usize {
        self.rgb
            .chunks_exact(3)
            .filter(|p| (p[0] as i32 - c.0 as i32).abs() <= tol && (p[1] as i32 - c.1 as i32).abs() <= tol && (p[2] as i32 - c.2 as i32).abs() <= tol)
            .count()
    }
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

#[test]
fn snippet_frac_with_boxes() {
    let t = TempDir::new("render-snippet");
    let r = Renderer::with_dirs(&real_config().join("templates/tex"), &t.path().join("cache"));
    let job = RenderJob::Snippet { latex: r"\frac{\square}{\square}".into(), math: true, display: false };
    assert!(source(&job).contains("standalone"));

    let t0 = Instant::now();
    let out = r.render_blocking(job.clone()).expect("render");
    let cold = ms(t0);
    assert!(out.png.starts_with(t.path()), "cached in the temp dir");
    let img = load(&out.png);
    println!("frac: {}×{} px for {:.1}×{:.1} pt, {} ink px", img.w, img.h, out.width, out.height, img.ink());
    assert_eq!(img.w, (out.width as f64 * 2.0).ceil() as u32, "2× the points");
    assert_eq!(img.h, (out.height as f64 * 2.0).ceil() as u32);
    assert!(out.width > 8.0 && out.width < 60.0, "a small fraction: {}", out.width);
    assert!(out.height > 15.0 && out.height < 60.0, "{}", out.height);
    assert!(img.ink() > 50, "not blank");
    // the tab stops are blue (ntexBlue #1E47B8, antialiased)
    assert!(img.near((0x1E, 0x47, 0xB8), 40) > 10, "blue boxes");

    // warm: the same job is a cache hit
    let t1 = Instant::now();
    let again = r.render_blocking(job).unwrap();
    let hit = ms(t1);
    assert_eq!(again, out);
    // warm tectonic, new source
    let t2 = Instant::now();
    let other = r.render_blocking(RenderJob::Snippet { latex: r"\sqrt{\square}".into(), math: true, display: true }).unwrap();
    let warm = ms(t2);
    assert_ne!(other.png, out.png);
    // text, not maths
    let txt = r.render_blocking(RenderJob::Snippet { latex: r"\textbf{Bold} words".into(), math: false, display: false }).unwrap();
    assert!(load(&txt.png).ink() > 50);
    println!("render snippet: cold {cold:.0} ms, warm (new source) {warm:.0} ms, cached {hit:.1} ms");

    // a TeX error comes back as the first ! line
    let err = r.render_blocking(RenderJob::Snippet { latex: r"\frac{\undefinedmacro}{2}".into(), math: true, display: false }).unwrap_err();
    assert_eq!(err, "! Undefined control sequence.");
    let err = r.render_blocking(RenderJob::Snippet { latex: r"x^".into(), math: true, display: false }).unwrap_err();
    assert!(err.starts_with('!'), "{err}");
    println!("errors: {err}");

    // nothing left behind but the cache
    let leftovers: Vec<_> = std::fs::read_dir(t.path().join("cache")).unwrap().flatten().map(|e| e.file_name()).filter(|n| !n.to_string_lossy().ends_with(".png") && !n.to_string_lossy().ends_with(".json")).collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn notes_document_with_a_remark() {
    let t = TempDir::new("render-doc");
    let r = Renderer::with_dirs(&real_config().join("templates/tex"), &t.path().join("cache"));
    let tex = r"\documentclass[11pt]{article}
\usepackage{ntex-notes}
\pagestyle{empty}
\begin{document}
\begin{remark}
A remark in the notes theme, with $\frac{a}{b}$.
\end{remark}
\end{document}
";
    let t0 = Instant::now();
    let out = r.render_blocking(RenderJob::Document { tex: tex.into() }).expect("document renders");
    let cold = ms(t0);
    let img = load(&out.png);
    println!("document: {}×{} px for {:.0}×{:.0} pt, {} ink px", img.w, img.h, out.width, out.height, img.ink());
    assert!((out.width - 612.0).abs() < 1.0, "a letter page: {}", out.width);
    assert_eq!(img.w, 1224);
    assert!(img.ink() > 500, "not blank");
    // the remark box: green fill (ntexGreenBack E5F5EA) and its rule (238B45)
    assert!(img.near((0xE5, 0xF5, 0xEA), 3) > 10_000, "the green remark fill");
    assert!(img.near((0x23, 0x8B, 0x45), 20) > 200, "the green rule");
    let t1 = Instant::now();
    r.render_blocking(RenderJob::Document { tex: tex.replace("A remark", "Another remark") }).unwrap();
    println!("render document: cold {cold:.0} ms, warm (new source) {:.0} ms", ms(t1));
}

#[test]
fn newer_render_supersedes_older() {
    let t = TempDir::new("render-race");
    let r = Renderer::with_dirs(&real_config().join("templates/tex"), &t.path().join("cache"));
    let a = r.render(RenderJob::Snippet { latex: r"\alpha+\square".into(), math: true, display: false });
    let b = r.render(RenderJob::Snippet { latex: r"\beta+\square".into(), math: true, display: false });
    let rb = b.recv().unwrap();
    assert!(rb.is_ok(), "{rb:?}");
    // the older one either finished first or was stopped; never an odd error
    match a.recv().unwrap() {
        Ok(_) => {}
        Err(e) => assert_eq!(e, "superseded"),
    }
}

#[test]
fn rasterises_an_existing_pdf() {
    // pdf_to_png on its own, with a PDF tectonic makes here
    let t = TempDir::new("render-pdf");
    let tex = t.path().join("a.tex");
    std::fs::write(&tex, "\\documentclass{standalone}\\begin{document}Hi\\end{document}\n").unwrap();
    let pdf = ntex_studio_io::render::compile(&tex, t.path(), &real_config().join("templates/tex"), |_| {}).unwrap();
    let png = t.path().join("a.png");
    let (w, h) = pdf_to_png(&pdf, &png, 2.0).unwrap();
    let img = load(&png);
    assert_eq!(img.w, (w * 2.0).ceil() as u32);
    assert_eq!(img.h, (h * 2.0).ceil() as u32);
    assert!(img.ink() > 20);
}
