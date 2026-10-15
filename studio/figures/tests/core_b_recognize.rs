//! FIG-CORE-B: `recognize` and `hit`. Scenes are built by hand (no layout).
//! The accuracy tests generate noisy trackpad strokes with a seeded PRNG and
//! require ≥ 95% per class; run with `--nocapture` to see the numbers.
//! `RECOG_SEED=k` draws a different set of cases; `RECOG_ALLMISS=1` lists
//! every miss.
//!
//! The trackpad model (`humanize`): uneven speed (a smooth speed profile ×
//! per-step noise), low-frequency wobble (two sinusoids per axis, 0–3% of
//! the size for loops), per-point Gaussian jitter with σ = 1.4–2.8% of the
//! size per axis (so the typical displacement is 2–4% of the size), random
//! rotation, sizes 1–4 cm, 30–200 points, and loops that overshoot (up to
//! 15%) or stop short (up to 7%). Polygons also get irregular vertices (±8%
//! radius, ±4.6° angle) and, half the time, slightly rounded corners (3–8%
//! of a side).
use ntex_figures::model::*;
use ntex_figures::recognize::{hit, obj_name, recognize};
use std::f32::consts::{PI, TAU};

// ---------------------------------------------------------------------------
// deterministic PRNG (splitmix64)

struct Rng(u64);
impl Rng {
    /// A generator for one test class; `RECOG_SEED=k` shifts every class to
    /// a fresh set of cases (the accuracy must hold for any seed).
    fn seeded(class: u64) -> Rng {
        let extra: u64 = std::env::var("RECOG_SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
        Rng(class ^ extra.wrapping_mul(0x9E37_79B9_7F4A_7C15))
    }
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// uniform [0, 1)
    fn f(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
    fn int(&mut self, a: i32, b: i32) -> i32 {
        a + (self.next() % ((b - a + 1) as u64)) as i32
    }
    fn gauss(&mut self) -> f32 {
        let u = self.f().max(1e-7);
        let v = self.f();
        (-2.0 * u.ln()).sqrt() * (TAU * v).cos()
    }
    fn coin(&mut self, p: f32) -> bool {
        self.f() < p
    }
}

fn pt(x: f32, y: f32) -> Pt {
    Pt { x, y }
}
fn rect(cx: f32, cy: f32, w: f32, h: f32) -> Rect {
    Rect { min: pt(cx - w / 2.0, cy - h / 2.0), max: pt(cx + w / 2.0, cy + h / 2.0) }
}

// ---------------------------------------------------------------------------
// the trackpad: an ideal curve c(t), t ∈ [0,1], sampled the way a hand does

/// Sample an ideal curve with uneven speed, low-frequency wobble and
/// per-point Gaussian jitter (σ = `jit` × `size`, so the mean offset is
/// ≈ 1.25·σ). Returns `n` points.
fn humanize(rng: &mut Rng, c: &dyn Fn(f32) -> Pt, n: usize, size: f32, jit: f32, wobble: f32) -> Vec<Pt> {
    // uneven speed: smooth speed profile plus per-step noise
    let (f, ph, amp) = (rng.range(0.5, 2.5), rng.range(0.0, TAU), rng.range(0.2, 0.6));
    let mut ts = vec![0.0f32];
    for i in 1..n {
        let u = i as f32 / n as f32;
        let v = (1.0 + amp * (TAU * f * u + ph).sin()) * rng.range(0.6, 1.4);
        let last = *ts.last().unwrap();
        ts.push(last + v.max(0.05));
    }
    let tmax = *ts.last().unwrap();
    // low-frequency wobble: two sinusoids per axis
    let w: Vec<(f32, f32, f32)> = (0..4).map(|_| (rng.range(0.5, 3.0), rng.range(0.0, TAU), rng.range(0.3, 1.0))).collect();
    ts.iter()
        .map(|&t| {
            let t = t / tmax;
            let p = c(t);
            let wx = wobble * size * (w[0].2 * (TAU * w[0].0 * t + w[0].1).sin() + w[1].2 * (TAU * w[1].0 * t + w[1].1).sin()) / 2.0;
            let wy = wobble * size * (w[2].2 * (TAU * w[2].0 * t + w[2].1).sin() + w[3].2 * (TAU * w[3].0 * t + w[3].1).sin()) / 2.0;
            pt(p.x + wx + jit * size * rng.gauss(), p.y + wy + jit * size * rng.gauss())
        })
        .collect()
}

fn bbox_wh(pts: &[Pt]) -> (f32, f32) {
    let (mut a, mut b, mut c, mut d) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in pts {
        a = a.min(p.x);
        b = b.min(p.y);
        c = c.max(p.x);
        d = d.max(p.y);
    }
    (c - a, d - b)
}

/// Random closure: the loop's parameter range [t0, t0 + span], span a bit
/// under (gap) or over (overshoot) one turn.
fn closure(rng: &mut Rng) -> f32 {
    if rng.coin(0.5) { 1.0 + rng.range(0.0, 0.15) } else { 1.0 - rng.range(0.0, 0.07) }
}

/// Ellipse with semi-axes (a, b), rotated by rot, centred at (cx, cy).
fn ellipse_stroke(rng: &mut Rng, cx: f32, cy: f32, a: f32, b: f32, rot: f32) -> Vec<Pt> {
    let t0 = rng.range(0.0, TAU);
    let dir = if rng.coin(0.5) { 1.0 } else { -1.0 };
    let span = closure(rng);
    let (cr, sr) = (rot.cos(), rot.sin());
    let c = move |t: f32| {
        let th = t0 + dir * TAU * span * t;
        let (x, y) = (a * th.cos(), b * th.sin());
        pt(cx + x * cr - y * sr, cy + x * sr + y * cr)
    };
    let n = rng.int(30, 200) as usize;
    let size = 2.0 * a.max(b);
    let jit = rng.range(0.014, 0.028);
    let wob = rng.range(0.0, 0.03);
    humanize(rng, &c, n, size, jit, wob)
}

/// A wobbly n-gon (slightly irregular vertices, optionally rounded corners),
/// drawn from a random point on its perimeter.
fn polygon_stroke(rng: &mut Rng, n: usize, cx: f32, cy: f32, r: f32) -> Vec<Pt> {
    let rot = rng.range(0.0, TAU);
    let mut v: Vec<Pt> = (0..n)
        .map(|k| {
            let th = rot + TAU * k as f32 / n as f32 + rng.range(-0.08, 0.08);
            let rr = r * rng.range(0.92, 1.08);
            pt(cx + rr * th.cos(), cy + rr * th.sin())
        })
        .collect();
    if rng.coin(0.5) {
        v.reverse();
    }
    // rounded corners: replace each corner by a quadratic curve of radius ρ
    let side = 2.0 * r * (PI / n as f32).sin();
    let rho = if rng.coin(0.5) { rng.range(0.03, 0.08) * side } else { 0.0 };
    let mut ideal: Vec<Pt> = Vec::new();
    for k in 0..n {
        let (p, c, q) = (v[(k + n - 1) % n], v[k], v[(k + 1) % n]);
        if rho <= 0.0 {
            ideal.push(c);
            continue;
        }
        let toward = |a: Pt, b: Pt, d: f32| {
            let l = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
            pt(a.x + (b.x - a.x) * d / l, a.y + (b.y - a.y) * d / l)
        };
        let (s, e) = (toward(c, p, rho), toward(c, q, rho));
        for i in 0..=6 {
            let t = i as f32 / 6.0;
            let u = 1.0 - t;
            ideal.push(pt(u * u * s.x + 2.0 * u * t * c.x + t * t * e.x, u * u * s.y + 2.0 * u * t * c.y + t * t * e.y));
        }
    }
    // perimeter parametrisation
    let m = ideal.len();
    let mut cum = vec![0.0f32];
    for i in 0..m {
        let (a, b) = (ideal[i], ideal[(i + 1) % m]);
        cum.push(cum[i] + ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt());
    }
    let per = cum[m];
    let s0 = rng.range(0.0, per);
    let span = closure(rng);
    let at = move |t: f32| {
        let s = (s0 + span * per * t).rem_euclid(per);
        let i = cum.partition_point(|&c| c <= s).saturating_sub(1).min(m - 1);
        let l = cum[i + 1] - cum[i];
        let f = if l > 0.0 { (s - cum[i]) / l } else { 0.0 };
        let (a, b) = (ideal[i], ideal[(i + 1) % m]);
        pt(a.x + (b.x - a.x) * f, a.y + (b.y - a.y) * f)
    };
    let np = rng.int(30, 200) as usize;
    let jit = rng.range(0.014, 0.028);
    let wob = rng.range(0.0, 0.02);
    humanize(rng, &at, np, 2.0 * r, jit, wob)
}

/// A sloppy stroke from a to b: slightly curved, overshooting a little.
fn arrow_stroke(rng: &mut Rng, a: Pt, b: Pt) -> Vec<Pt> {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l = (dx * dx + dy * dy).sqrt();
    let (nx, ny) = (-dy / l, dx / l);
    let bend = rng.range(-0.2, 0.2) * l;
    let over = rng.range(0.0, 0.2);
    let ctrl = pt((a.x + b.x) / 2.0 + nx * bend, (a.y + b.y) / 2.0 + ny * bend);
    let end = pt(b.x + dx / l * over, b.y + dy / l * over);
    let c = move |t: f32| {
        let u = 1.0 - t;
        pt(u * u * a.x + 2.0 * u * t * ctrl.x + t * t * end.x, u * u * a.y + 2.0 * u * t * ctrl.y + t * t * end.y)
    };
    let n = rng.int(30, 200) as usize;
    let jit = rng.range(0.005, 0.015);
    let wob = rng.range(0.0, 0.02);
    humanize(rng, &c, n, l, jit, wob)
}

/// A point in the box padded by `pad`.
fn in_box(rng: &mut Rng, r: &Rect, pad: f32) -> Pt {
    pt(rng.range(r.min.x - pad, r.max.x + pad), rng.range(r.min.y - pad, r.max.y + pad))
}

// ---------------------------------------------------------------------------
// hand-built scenes

/// A closed blob outline: an ellipse as four cubics (κ = 0.5523).
fn blob_path(name: &str, cx: f32, cy: f32, a: f32, b: f32) -> Draw {
    let k = 0.552_284_8;
    let p = |x: f32, y: f32| pt(cx + x, cy + y);
    Draw::Path {
        obj: ObjRef::Blob(name.into()),
        start: p(a, 0.0),
        segs: vec![
            Seg::Cubic(p(a, k * b), p(k * a, b), p(0.0, b)),
            Seg::Cubic(p(-k * a, b), p(-a, k * b), p(-a, 0.0)),
            Seg::Cubic(p(-a, -k * b), p(-k * a, -b), p(0.0, -b)),
            Seg::Cubic(p(k * a, -b), p(a, -k * b), p(a, 0.0)),
        ],
        closed: true,
        stroke: None,
        fill: None,
        fill_opacity: 0.0,
        start_tip: None,
        end_tip: None,
    }
}

fn line_path(obj: ObjRef, a: Pt, b: Pt) -> Draw {
    Draw::Path { obj, start: a, segs: vec![Seg::Line(b)], closed: false, stroke: None, fill: None, fill_opacity: 0.0, start_tip: None, end_tip: None }
}

/// Nodes on a 3×3 grid of cells (names "N{col}{row}"), boxes w×h.
fn grid_scene(rng: &mut Rng) -> Scene {
    let cell = (2.4, 1.8);
    let mut s = Scene { cell, ..Default::default() };
    for col in 0..3 {
        for row in 0..3 {
            let c = pt(col as f32 * cell.0, -(row as f32) * cell.1);
            let (w, h) = (rng.range(0.4, 0.9), rng.range(0.35, 0.6));
            s.anchors.push((ObjRef::Node(format!("N{col}{row}")), c, rect(c.x, c.y, w, h)));
        }
    }
    s
}

/// Two blobs side by side (G at x=0, H at x=5), three elements each.
fn blob_scene() -> Scene {
    let mut s = Scene::default();
    for (bn, cx) in [("G", 0.0f32), ("H", 5.0)] {
        s.draws.push(blob_path(bn, cx, 0.0, 1.6, 1.4));
        s.anchors.push((ObjRef::Blob(bn.into()), pt(cx, 0.0), rect(cx, 0.0, 3.2, 2.8)));
        for (i, (ex, ey)) in [(-0.6f32, 0.5f32), (0.6, 0.4), (0.0, -0.6)].iter().enumerate() {
            let c = pt(cx + ex, *ey);
            s.anchors.push((ObjRef::Element(format!("{bn}{i}")), c, rect(c.x, c.y, 0.3, 0.3)));
        }
    }
    s
}

// ---------------------------------------------------------------------------
// accuracy harness

fn report(class: &str, ok: usize, total: usize, fails: &[String]) -> f32 {
    let acc = ok as f32 / total as f32;
    eprintln!("[recognize] {class:<28} {ok:>4}/{total:<4} = {:.1}%", acc * 100.0);
    let shown = if std::env::var("RECOG_ALLMISS").is_ok() { usize::MAX } else { 5 };
    for f in fails.iter().take(shown) {
        eprintln!("    miss: {f}");
    }
    acc
}

const CASES: usize = 400;

#[test]
fn accuracy_circles_and_ellipses() {
    let mut rng = Rng::seeded(0xC1AC1E);
    let scene = Scene::default();
    let (mut ok, mut fails) = (0, Vec::new());
    let (mut shape_ok, mut shape_n) = (0, 0);
    for i in 0..CASES {
        let size = rng.range(1.0, 4.0); // the larger diameter
        let aspect = if i % 2 == 0 { rng.range(1.0, 1.15) } else { rng.range(1.7, 2.6) };
        let (a, b) = if rng.coin(0.5) { (size / 2.0, size / 2.0 / aspect) } else { (size / 2.0 / aspect, size / 2.0) };
        // round ones at any rotation; elongated ones roughly axis-aligned
        let rot = if aspect < 1.2 { rng.range(0.0, TAU) } else { rng.range(-0.15, 0.15) };
        let (cx, cy) = (rng.range(-5.0, 5.0), rng.range(-5.0, 5.0));
        let stroke = ellipse_stroke(&mut rng, cx, cy, a, b, rot);
        // the noise-free ellipse's bbox
        let ideal: Vec<Pt> = (0..360)
            .map(|k| {
                let th = k as f32 * TAU / 360.0;
                let (x, y) = (a * th.cos(), b * th.sin());
                pt(x * rot.cos() - y * rot.sin(), x * rot.sin() + y * rot.cos())
            })
            .collect();
        let (iw, ih) = bbox_wh(&ideal);
        let want = if iw > 1.6 * ih {
            Some(BlobShape::Wide)
        } else if ih > 1.6 * iw {
            Some(BlobShape::Tall)
        } else if iw.max(ih) / iw.min(ih) < 1.2 {
            Some(BlobShape::Round)
        } else {
            None
        };
        match recognize(&stroke, &scene) {
            Recognized::Blob { size: (w, h), shape, center, .. } => {
                let size_ok = (w / iw - 1.0).abs() <= 0.25 && (h / ih - 1.0).abs() <= 0.25;
                let centre_ok = ((center.x - cx).powi(2) + (center.y - cy).powi(2)).sqrt() < 0.15 * size;
                let sh_ok = want.is_none_or(|s| s == shape);
                if want.is_some() {
                    shape_n += 1;
                    if sh_ok {
                        shape_ok += 1;
                    }
                }
                if size_ok && centre_ok && sh_ok {
                    ok += 1;
                } else {
                    fails.push(format!("#{i} aspect {aspect:.2} got {w:.2}x{h:.2} {shape:?} want {iw:.2}x{ih:.2} {want:?}"));
                }
            }
            other => fails.push(format!("#{i} aspect {aspect:.2} size {size:.2} n {} → {other:?}", stroke.len())),
        }
    }
    let acc = report("circles/ellipses → Blob", ok, CASES, &fails);
    eprintln!("[recognize]   (shape right in {shape_ok}/{shape_n})");
    assert!(acc >= 0.95, "circles/ellipses accuracy {acc}");
}

#[test]
fn accuracy_polygons() {
    let scene = Scene::default();
    let mut worst = 1.0f32;
    for n in 3..=6usize {
        let mut rng = Rng::seeded(0x9070 + n as u64);
        let (mut ok, mut fails) = (0, Vec::new());
        for i in 0..CASES {
            let r = rng.range(0.5, 2.0); // diameter 1–4 cm
            let (cx, cy) = (rng.range(-5.0, 5.0), rng.range(-5.0, 5.0));
            let stroke = polygon_stroke(&mut rng, n, cx, cy, r);
            match recognize(&stroke, &scene) {
                Recognized::NGon { n: got, center, radius } if got as usize == n => {
                    let c_ok = ((center.x - cx).powi(2) + (center.y - cy).powi(2)).sqrt() < 0.2 * r;
                    let r_ok = (radius / r - 1.0).abs() < 0.2;
                    if c_ok && r_ok {
                        ok += 1;
                    } else {
                        fails.push(format!("#{i} centre/radius off: r {radius:.2} want {r:.2}"));
                    }
                }
                other => fails.push(format!("#{i} r {r:.2} pts {} → {other:?}", stroke.len())),
            }
        }
        let acc = report(&format!("{n}-gons → NGon({n})"), ok, CASES, &fails);
        worst = worst.min(acc);
    }
    assert!(worst >= 0.95, "polygon accuracy {worst}");
}

#[test]
fn accuracy_arrows_between_nodes() {
    let mut rng = Rng::seeded(0xA770);
    let (mut ok, mut fails) = (0, Vec::new());
    for i in 0..CASES {
        let scene = grid_scene(&mut rng);
        let a = rng.int(0, 8) as usize;
        let mut b = rng.int(0, 7) as usize;
        if b >= a {
            b += 1;
        }
        let (oa, _, ra) = scene.anchors[a].clone();
        let (ob, _, rb) = scene.anchors[b].clone();
        let pa = in_box(&mut rng, &ra, 0.25);
        let pb = in_box(&mut rng, &rb, 0.25);
        let stroke = arrow_stroke(&mut rng, pa, pb);
        let want = Recognized::Arrow { from: obj_name(&oa).unwrap(), to: obj_name(&ob).unwrap() };
        let got = recognize(&stroke, &scene);
        if got == want {
            ok += 1;
        } else {
            fails.push(format!("#{i} {want:?} → {got:?}"));
        }
    }
    let acc = report("arrows node→node", ok, CASES, &fails);
    assert!(acc >= 0.95, "arrow accuracy {acc}");
}

#[test]
fn accuracy_arrows_between_elements() {
    let mut rng = Rng::seeded(0xE1E7);
    let scene = blob_scene();
    let elems: Vec<(ObjRef, Rect)> =
        scene.anchors.iter().filter(|(o, _, _)| matches!(o, ObjRef::Element(_))).map(|(o, _, r)| (o.clone(), r.clone())).collect();
    let (mut ok, mut fails) = (0, Vec::new());
    for i in 0..CASES {
        let a = rng.int(0, elems.len() as i32 - 1) as usize;
        let mut b = rng.int(0, elems.len() as i32 - 2) as usize;
        if b >= a {
            b += 1;
        }
        // elements are 0.3 cm and ~1 cm apart: a hand lands within 0.15 cm
        let pa = in_box(&mut rng, &elems[a].1, 0.15);
        let pb = in_box(&mut rng, &elems[b].1, 0.15);
        let stroke = arrow_stroke(&mut rng, pa, pb);
        let want = Recognized::Arrow { from: obj_name(&elems[a].0).unwrap(), to: obj_name(&elems[b].0).unwrap() };
        let got = recognize(&stroke, &scene);
        if got == want {
            ok += 1;
        } else {
            fails.push(format!("#{i} {want:?} → {got:?}"));
        }
    }
    let acc = report("arrows element→element", ok, CASES, &fails);
    assert!(acc >= 0.95, "element arrow accuracy {acc}");
}

#[test]
fn accuracy_taps() {
    let mut rng = Rng::seeded(0x7A95);
    let (mut ok, mut fails) = (0, Vec::new());
    for i in 0..CASES {
        let cell = if rng.coin(0.5) { (2.4, 1.8) } else { (rng.range(1.0, 3.0), rng.range(1.0, 3.0)) };
        let scene = Scene { cell, ..Default::default() };
        let (col, row) = (rng.int(-4, 6), rng.int(-4, 6));
        // somewhere within the cell's snapping area (not on the boundary)
        let x = col as f32 * cell.0 + rng.range(-0.4, 0.4) * cell.0;
        let y = -(row as f32) * cell.1 + rng.range(-0.4, 0.4) * cell.1;
        let n = rng.int(1, 3);
        let stroke: Vec<Pt> = (0..n).map(|_| pt(x + rng.range(-0.05, 0.05), y + rng.range(-0.05, 0.05))).collect();
        let got = recognize(&stroke, &scene);
        let want = Recognized::Node(Pos::Cell(col as f32, row as f32));
        if got == want {
            ok += 1;
        } else {
            fails.push(format!("#{i} {want:?} → {got:?}"));
        }
    }
    let acc = report("taps → Node(cell)", ok, CASES, &fails);
    assert!(acc >= 0.95, "tap accuracy {acc}");
}

#[test]
fn accuracy_nothing() {
    let mut rng = Rng::seeded(0x0707);
    // objects far from where the scribbles happen
    let mut scene = Scene::default();
    scene.anchors.push((ObjRef::Node("A".into()), pt(-30.0, 0.0), rect(-30.0, 0.0, 0.6, 0.5)));
    scene.anchors.push((ObjRef::Node("B".into()), pt(30.0, 0.0), rect(30.0, 0.0, 0.6, 0.5)));
    let (mut ok, mut fails) = (0, Vec::new());
    for i in 0..CASES {
        let size = rng.range(1.0, 4.0);
        let (cx, cy) = (rng.range(-5.0, 5.0), rng.range(-5.0, 5.0));
        let rot = rng.range(0.0, TAU);
        let (cr, sr) = (rot.cos(), rot.sin());
        let place = move |x: f32, y: f32| pt(cx + x * cr - y * sr, cy + x * sr + y * cr);
        let kind = i % 4;
        let stroke = match kind {
            0 => {
                // zig-zag
                let teeth = rng.int(2, 6) as f32;
                let amp = rng.range(0.15, 0.4) * size;
                let c = move |t: f32| {
                    let ph = (t * teeth * 2.0).fract();
                    let tri = if ph < 0.5 { ph * 2.0 } else { 2.0 - ph * 2.0 };
                    place((t - 0.5) * size, amp * (tri - 0.5))
                };
                { let n = rng.int(30, 200) as usize; humanize(&mut rng, &c, n, size, 0.015, 0.01) }
            }
            1 => {
                // partial arc, 60°–220°
                let sweep = rng.range(60.0f32, 220.0).to_radians();
                let r = size / 2.0;
                let c = move |t: f32| {
                    let th = sweep * t;
                    place(r * th.cos(), r * th.sin())
                };
                { let n = rng.int(30, 200) as usize; let j = rng.range(0.01, 0.025); humanize(&mut rng, &c, n, size, j, 0.01) }
            }
            2 => {
                // straight-ish line
                let c = move |t: f32| place((t - 0.5) * size, 0.0);
                { let n = rng.int(30, 200) as usize; humanize(&mut rng, &c, n, size, 0.01, 0.02) }
            }
            _ => {
                // S-curve
                let amp = rng.range(0.15, 0.35) * size;
                let c = move |t: f32| place((t - 0.5) * size, amp * (TAU * t).sin());
                { let n = rng.int(30, 200) as usize; humanize(&mut rng, &c, n, size, 0.015, 0.01) }
            }
        };
        let got = recognize(&stroke, &scene);
        if got == Recognized::Nothing {
            ok += 1;
        } else {
            fails.push(format!("#{i} kind {kind} size {size:.2} → {got:?}"));
        }
    }
    let acc = report("scribbles/arcs/lines → Nothing", ok, CASES, &fails);
    assert!(acc >= 0.95, "nothing accuracy {acc}");
}

// ---------------------------------------------------------------------------
// deterministic tests

#[test]
fn obj_names() {
    assert_eq!(obj_name(&ObjRef::Node("A".into())), Some("A".into()));
    assert_eq!(obj_name(&ObjRef::Element("x".into())), Some("x".into()));
    assert_eq!(obj_name(&ObjRef::Blob("G".into())), Some("G".into()));
    assert_eq!(obj_name(&ObjRef::CayleyVertex("C".into(), "rs".into())), Some("C.rs".into()));
    assert_eq!(obj_name(&ObjRef::NGonVertex("P".into(), 3)), Some("P.3".into()));
    assert_eq!(obj_name(&ObjRef::Arrow(2)), None);
    assert_eq!(obj_name(&ObjRef::NGon("P".into())), None);
    assert_eq!(obj_name(&ObjRef::CayleyEdge("C".into(), 0)), None);
    assert_eq!(obj_name(&ObjRef::Text(0)), None);
}

/// Parent blob G (big) containing child K, element x inside K, node A
/// outside, and an arrow A → x.
fn nested_scene() -> Scene {
    let mut s = Scene::default();
    s.draws.push(blob_path("G", 0.0, 0.0, 3.0, 2.0));
    s.draws.push(blob_path("K", 1.5, 0.0, 1.0, 0.9));
    s.anchors.push((ObjRef::Blob("G".into()), pt(0.0, 0.0), rect(0.0, 0.0, 6.0, 4.0)));
    s.anchors.push((ObjRef::Blob("K".into()), pt(1.5, 0.0), rect(1.5, 0.0, 2.0, 1.8)));
    s.anchors.push((ObjRef::Element("x".into()), pt(1.6, 0.2), rect(1.6, 0.2, 0.3, 0.3)));
    s.anchors.push((ObjRef::Element("y".into()), pt(-1.8, 0.5), rect(-1.8, 0.5, 0.3, 0.3)));
    s.anchors.push((ObjRef::Node("A".into()), pt(6.0, 0.2), rect(6.0, 0.2, 0.6, 0.5)));
    s.draws.push(line_path(ObjRef::Arrow(7), pt(5.6, 0.2), pt(1.9, 0.2)));
    s
}

#[test]
fn hit_ordering() {
    let s = nested_scene();
    // node beats everything
    assert_eq!(hit(&s, pt(6.0, 0.2)), Some(ObjRef::Node("A".into())));
    // node box padding (~0.1)
    assert_eq!(hit(&s, pt(6.36, 0.2)), Some(ObjRef::Node("A".into())));
    // element inside the child blob, on the arrow's end: element first
    assert_eq!(hit(&s, pt(1.65, 0.2)), Some(ObjRef::Element("x".into())));
    // the arrow outside the blobs, and on top of blob K
    assert_eq!(hit(&s, pt(4.0, 0.25)), Some(ObjRef::Arrow(7)));
    assert_eq!(hit(&s, pt(2.2, 0.2)), Some(ObjRef::Arrow(7)));
    // inside K (not on the arrow): child beats parent
    assert_eq!(hit(&s, pt(1.3, -0.5)), Some(ObjRef::Blob("K".into())));
    // inside G only
    assert_eq!(hit(&s, pt(-2.0, -0.8)), Some(ObjRef::Blob("G".into())));
    // empty space
    assert_eq!(hit(&s, pt(4.0, 2.5)), None);
    assert_eq!(hit(&Scene::default(), pt(0.0, 0.0)), None);
}

#[test]
fn hit_smallest_box_and_dots() {
    let mut s = Scene::default();
    // a text box overlapping a smaller node: the smaller wins
    s.anchors.push((ObjRef::Text(0), pt(0.0, 0.0), rect(0.0, 0.0, 3.0, 0.6)));
    s.anchors.push((ObjRef::Node("B".into()), pt(0.5, 0.0), rect(0.5, 0.0, 0.4, 0.4)));
    assert_eq!(hit(&s, pt(0.5, 0.0)), Some(ObjRef::Node("B".into())));
    assert_eq!(hit(&s, pt(-1.0, 0.0)), Some(ObjRef::Text(0)));
    // a Cayley vertex dot and an edge
    s.draws.push(Draw::Dot { obj: ObjRef::CayleyVertex("C".into(), "e".into()), at: pt(5.0, 0.0), r: 0.06, color: "ink".into() });
    s.draws.push(line_path(ObjRef::CayleyEdge("C".into(), 0), pt(5.0, 0.0), pt(7.0, 0.0)));
    assert_eq!(hit(&s, pt(5.05, 0.02)), Some(ObjRef::CayleyVertex("C".into(), "e".into())));
    assert_eq!(hit(&s, pt(6.0, 0.1)), Some(ObjRef::CayleyEdge("C".into(), 0)));
    assert_eq!(hit(&s, pt(6.0, 0.3)), None);
}

#[test]
fn hit_ngon_outline() {
    let mut s = Scene::default();
    // a square n-gon as a closed line path
    s.draws.push(Draw::Path {
        obj: ObjRef::NGon("P".into()),
        start: pt(1.0, 1.0),
        segs: vec![Seg::Line(pt(-1.0, 1.0)), Seg::Line(pt(-1.0, -1.0)), Seg::Line(pt(1.0, -1.0))],
        closed: true,
        stroke: None,
        fill: None,
        fill_opacity: 0.0,
        start_tip: None,
        end_tip: None,
    });
    s.anchors.push((ObjRef::NGonVertex("P".into(), 1), pt(1.0, 1.0), rect(1.0, 1.0, 0.2, 0.2)));
    assert_eq!(hit(&s, pt(0.0, 0.0)), Some(ObjRef::NGon("P".into())));
    assert_eq!(hit(&s, pt(1.0, 1.0)), Some(ObjRef::NGonVertex("P".into(), 1)));
    assert_eq!(hit(&s, pt(1.5, 0.0)), None);
}

fn circle(cx: f32, cy: f32, r: f32, n: usize) -> Vec<Pt> {
    (0..=n).map(|k| {
        let th = k as f32 * TAU / n as f32;
        pt(cx + r * th.cos(), cy + r * th.sin())
    }).collect()
}

#[test]
fn encloses_lists_what_is_inside() {
    let s = nested_scene();
    // a loop around K (and so x), not y, not A
    match recognize(&circle(1.5, 0.0, 1.2, 80), &s) {
        Recognized::Blob { encloses, shape, .. } => {
            assert_eq!(encloses, vec!["K".to_string(), "x".to_string()]);
            assert_eq!(shape, BlobShape::Round);
        }
        other => panic!("{other:?}"),
    }
    // a loop around everything: scene (anchor) order
    match recognize(&circle(1.5, 0.0, 5.5, 80), &s) {
        Recognized::Blob { encloses, .. } => assert_eq!(encloses, vec!["G", "K", "x", "y", "A"]),
        other => panic!("{other:?}"),
    }
    // a loop in empty space
    match recognize(&circle(10.0, 10.0, 1.0, 60), &s) {
        Recognized::Blob { encloses, .. } => assert!(encloses.is_empty()),
        other => panic!("{other:?}"),
    }
}

#[test]
fn clean_shapes() {
    let s = Scene::default();
    // a wide ellipse
    let e: Vec<Pt> = (0..=90).map(|k| {
        let th = k as f32 * TAU / 90.0;
        pt(2.0 * th.cos(), 0.8 * th.sin())
    }).collect();
    match recognize(&e, &s) {
        Recognized::Blob { shape, size, center, .. } => {
            assert_eq!(shape, BlobShape::Wide);
            // the box of the lightly smoothed loop: within 3%
            assert!((size.0 / 4.0 - 1.0).abs() < 0.03 && (size.1 / 1.6 - 1.0).abs() < 0.03, "{size:?}");
            assert!(center.x.abs() < 0.05 && center.y.abs() < 0.05);
        }
        other => panic!("{other:?}"),
    }
    // a tall one
    let t: Vec<Pt> = e.iter().map(|p| pt(p.y, p.x)).collect();
    assert!(matches!(recognize(&t, &s), Recognized::Blob { shape: BlobShape::Tall, .. }));
    // an exact square, 10 points per side
    let corners = [pt(0.0, 0.0), pt(2.0, 0.0), pt(2.0, 2.0), pt(0.0, 2.0)];
    let mut sq = Vec::new();
    for k in 0..4 {
        for i in 0..10 {
            let (a, b) = (corners[k], corners[(k + 1) % 4]);
            let t = i as f32 / 10.0;
            sq.push(pt(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t));
        }
    }
    sq.push(corners[0]);
    match recognize(&sq, &s) {
        Recognized::NGon { n, center, radius } => {
            assert_eq!(n, 4);
            assert!((center.x - 1.0).abs() < 0.1 && (center.y - 1.0).abs() < 0.1, "{center:?}");
            assert!((radius - 2f32.sqrt()).abs() < 0.15, "{radius}");
        }
        other => panic!("{other:?}"),
    }
    // a bean (kidney): a round loop with a deep dent on top
    let bean: Vec<Pt> = (0..=120)
        .map(|k| {
            let t = k as f32 * TAU / 120.0;
            let dent = 1.0 * (-((t - PI / 2.0) / 0.6).powi(2)).exp();
            pt(0.85 * t.cos(), t.sin() - dent)
        })
        .collect();
    let got = recognize(&bean, &s);
    assert!(matches!(got, Recognized::Blob { shape: BlobShape::Bean, .. }), "{got:?}");
}

#[test]
fn degenerate_strokes() {
    let s = Scene::default();
    assert_eq!(recognize(&[], &s), Recognized::Nothing);
    assert_eq!(recognize(&[pt(0.0, 0.0)], &s), Recognized::Node(Pos::Cell(0.0, 0.0)));
    assert_eq!(recognize(&[pt(2.5, -1.7), pt(2.5, -1.7)], &s), Recognized::Node(Pos::Cell(1.0, 1.0)));
    // unknown cell → (2.4, 1.8); known cell is used
    let s2 = Scene { cell: (1.0, 1.0), ..Default::default() };
    assert_eq!(recognize(&[pt(2.9, -4.1)], &s2), Recognized::Node(Pos::Cell(3.0, 4.0)));
    // NaN points are ignored
    assert_eq!(recognize(&[pt(f32::NAN, 0.0)], &s), Recognized::Nothing);
    // out-and-back line: not a loop
    let mut ob: Vec<Pt> = (0..30).map(|i| pt(i as f32 * 0.1, 0.0)).collect();
    ob.extend((0..30).rev().map(|i| pt(i as f32 * 0.1, 0.05)));
    assert_eq!(recognize(&ob, &s), Recognized::Nothing);
}

#[test]
fn arrow_rules() {
    let s = nested_scene();
    // A → x (element inside child blob K inside G): element wins
    let st: Vec<Pt> = (0..=40).map(|i| pt(6.0 - i as f32 * 0.11, 0.2)).collect();
    assert_eq!(recognize(&st, &s), Recognized::Arrow { from: "A".into(), to: "x".into() });
    // A → empty part of K: the child blob
    let st: Vec<Pt> = (0..=40).map(|i| pt(6.0 - i as f32 * (6.0 - 1.3) / 40.0, 0.2 - i as f32 * 0.7 / 40.0)).collect();
    assert_eq!(recognize(&st, &s), Recognized::Arrow { from: "A".into(), to: "K".into() });
    // A → empty part of G: the parent
    let st: Vec<Pt> = (0..=40).map(|i| pt(6.0 - i as f32 * 8.0 / 40.0, 0.2 - i as f32 * 1.3 / 40.0)).collect();
    assert_eq!(recognize(&st, &s), Recognized::Arrow { from: "A".into(), to: "G".into() });
    // same object at both ends (an open hook from A back to A): not an arrow
    let st: Vec<Pt> = (0..=40).map(|i| {
        let th = i as f32 / 40.0 * PI;
        pt(6.0 + 0.8 * th.sin(), 0.2 + 0.2 * th.cos() - 0.2)
    }).collect();
    assert_eq!(recognize(&st, &s), Recognized::Nothing);
    // A → empty space
    let st: Vec<Pt> = (0..=40).map(|i| pt(6.0 + i as f32 * 0.1, 0.2)).collect();
    assert_eq!(recognize(&st, &s), Recognized::Nothing);
    // Cayley and n-gon vertices are named "C.w" / "P.k"
    let mut s2 = Scene::default();
    s2.anchors.push((ObjRef::CayleyVertex("C".into(), "r".into()), pt(0.0, 0.0), rect(0.0, 0.0, 0.2, 0.2)));
    s2.anchors.push((ObjRef::NGonVertex("P".into(), 2), pt(3.0, 0.0), rect(3.0, 0.0, 0.2, 0.2)));
    let st: Vec<Pt> = (0..=30).map(|i| pt(i as f32 * 0.1, 0.0)).collect();
    assert_eq!(recognize(&st, &s2), Recognized::Arrow { from: "C.r".into(), to: "P.2".into() });
}

