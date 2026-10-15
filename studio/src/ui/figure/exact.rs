//! The exact render: the figure's TikZ document → tectonic → PDF → PNG,
//! with the studio's own `io::render::compile` and `pdf_to_png`, but
//! rasterised at the canvas's pixel density rather than a fixed 2× (at
//! 48 px/cm on a Retina screen 2× is visibly soft next to the draft).
//!
//! One render in flight per window: a newer one kills the older tectonic
//! (process group). Cached by the source, the theme's .sty files and the
//! scale in `<state>/figure-cache/`. Drop kills whatever is running.
use ntex_studio_io::render::{compile, hash, pdf_to_png};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq)]
pub struct Raster {
    pub png: PathBuf,
    /// The page in pt.
    pub w_pt: f32,
    pub h_pt: f32,
    /// Pixels per pt in the PNG.
    pub scale: f64,
}

pub struct ExactRenderer {
    inner: Arc<Inner>,
}

struct Inner {
    templates: PathBuf,
    cache: PathBuf,
    latest: AtomicU64,
    next: AtomicU64,
    running: Mutex<HashMap<u64, i32>>,
}

unsafe extern "C" {
    fn killpg(pgrp: i32, sig: i32) -> i32;
}

fn kill_group(pgid: i32) {
    if pgid > 0 {
        // SAFETY: signals a process group we started (tectonic's)
        unsafe {
            killpg(pgid, 9);
        }
    }
}

impl ExactRenderer {
    pub fn new(templates: &Path, state: &Path) -> ExactRenderer {
        ExactRenderer {
            inner: Arc::new(Inner {
                templates: templates.to_path_buf(),
                cache: state.join("figure-cache"),
                latest: AtomicU64::new(0),
                next: AtomicU64::new(1),
                running: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Start a render; an older one still compiling is stopped.
    pub fn render(&self, tex: String, scale: f64) -> Receiver<Result<Raster, String>> {
        let (tx, rx) = channel();
        let inner = self.inner.clone();
        let generation = inner.next.fetch_add(1, Ordering::SeqCst);
        inner.latest.store(generation, Ordering::SeqCst);
        if let Ok(running) = inner.running.lock() {
            for (g, pgid) in running.iter() {
                if *g < generation {
                    kill_group(*pgid);
                }
            }
        }
        let spawned = std::thread::Builder::new().name("ntex-fig-render".into()).spawn(move || {
            let r = inner.run(&tex, scale, generation);
            let _ = tx.send(r);
        });
        if spawned.is_err() {
            let (tx, rx) = channel();
            let _ = tx.send(Err("couldn't start the render".into()));
            return rx;
        }
        rx
    }
}

impl Drop for ExactRenderer {
    fn drop(&mut self) {
        if let Ok(running) = self.inner.running.lock() {
            for pgid in running.values() {
                kill_group(*pgid);
            }
        }
    }
}

impl Inner {
    fn key(&self, tex: &str, scale: f64) -> u64 {
        let mut parts: Vec<Vec<u8>> = vec![b"ntex-figure-v1".to_vec(), tex.as_bytes().to_vec(), scale.to_string().into_bytes()];
        if let Ok(rd) = std::fs::read_dir(&self.templates) {
            let mut names: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "sty")).collect();
            names.sort();
            for p in names {
                parts.push(std::fs::read(&p).unwrap_or_default());
            }
        }
        let refs: Vec<&[u8]> = parts.iter().map(|v| v.as_slice()).collect();
        hash(&refs)
    }

    fn run(&self, tex: &str, scale: f64, generation: u64) -> Result<Raster, String> {
        let key = self.key(tex, scale);
        let png = self.cache.join(format!("{key:016x}.png"));
        let meta = self.cache.join(format!("{key:016x}.json"));
        if png.exists()
            && let Ok(m) = std::fs::read_to_string(&meta)
            && let Ok(v) = serde_json::from_str::<serde_json::Value>(&m)
            && let (Some(w), Some(h)) = (v["w"].as_f64(), v["h"].as_f64())
        {
            return Ok(Raster { png, w_pt: w as f32, h_pt: h as f32, scale });
        }
        std::fs::create_dir_all(&self.cache).map_err(|e| format!("can't make {}: {e}", self.cache.display()))?;
        let work = self.cache.join(format!("work-{key:016x}-{}-{generation}", std::process::id()));
        let _ = std::fs::remove_dir_all(&work);
        std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
        let res = (|| {
            let file = work.join("figure.tex");
            std::fs::write(&file, tex).map_err(|e| e.to_string())?;
            let pdf = compile(&file, &work, &self.templates, |pgid| {
                if let Ok(mut r) = self.running.lock() {
                    r.insert(generation, pgid);
                }
            });
            if let Ok(mut r) = self.running.lock() {
                r.remove(&generation);
            }
            if self.latest.load(Ordering::SeqCst) != generation {
                return Err("superseded".to_string());
            }
            let pdf = pdf?;
            let part = png.with_extension("png.part");
            let (w, h) = pdf_to_png(&pdf, &part, scale)?;
            std::fs::rename(&part, &png).map_err(|e| e.to_string())?;
            let _ = std::fs::write(&meta, serde_json::json!({ "w": w, "h": h }).to_string());
            Ok(Raster { png: png.clone(), w_pt: w as f32, h_pt: h as f32, scale })
        })();
        let _ = std::fs::remove_dir_all(&work);
        res
    }
}
