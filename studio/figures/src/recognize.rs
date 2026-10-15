//! Rough canvas gestures → objects (trackpad: clicks, drags, rough loops,
//! rough polygons). Pure geometry; thresholds in cm.
//!
//! `recognize` decides, in this order:
//! 1. **Tap**: one point, or path length and bbox diagonal both < `TAP` →
//!    `Node` on the nearest whole cell.
//! 2. **Closed loop**: after trimming overshoot (closest pair of points
//!    between the first and last quarter of the stroke), the remaining gap is
//!    < `LOOP_GAP` × bbox diagonal, the denoised loop winds once around its
//!    centre (≥ 300°) and is not a sliver (isoperimetric quotient
//!    ≥ `LOOP_ISO`). A gap that cut a corner gets the corner back (the two
//!    ends' tangents meet). Then corner detection on the turning angle plus
//!    a BIC model choice (see `corners`) decides `NGon` (3–9 corners) or
//!    `Blob` (shape from the box's aspect; `Bean` when notably non-convex;
//!    `encloses` = the nodes, elements and blobs whose anchor centre lies
//!    inside the stroke, in `scene.anchors` order).
//! 3. **Arrow**: start on one object, end on another (`endpoint`).
//! 4. Otherwise `Nothing`.
//!
//! `hit` returns the topmost object: labels/nodes/elements/vertices/dots
//! first, then open paths (arrows, Cayley edges), then closed shapes
//! (n-gons, blobs; the smallest containing one wins).
use crate::model::*;

// ---------------------------------------------------------------------------
// thresholds (cm unless stated)

/// A tap: path length and bbox diagonal both below this.
const TAP: f32 = 0.3;
/// Hit padding around anchor boxes for `hit`.
const HIT_PAD: f32 = 0.1;
/// Distance to an open path's curve that counts as on it, for `hit`.
const HIT_PATH: f32 = 0.15;
/// Padding around anchor boxes for arrow endpoints (generous: trackpad).
const ARROW_PAD: f32 = 0.35;
/// Distance outside a blob's outline that still counts as on it (arrows).
const ARROW_BLOB_PAD: f32 = 0.2;
/// When an arrow's raw endpoint misses, look back this fraction of the
/// stroke's length for the first point on an object (overshoot).
const ARROW_LOOKBACK: f32 = 0.15;
/// A loop's closing gap, relative to its bbox diagonal.
const LOOP_GAP: f32 = 0.25;
/// Minimal winding of a closed loop around its centre (degrees).
const LOOP_TURN_DEG: f32 = 300.0;
/// Minimal isoperimetric quotient 4πA/P² of a loop (circle 1, square 0.79,
/// equilateral triangle 0.60); rejects out-and-back slivers.
const LOOP_ISO: f32 = 0.2;
/// Declumping radius, relative to the loop's bbox diagonal.
const DECLUMP: f32 = 0.02;
/// Samples along a closed loop.
const N: usize = 64;
/// Corner strength is the turn between chords spanning this many samples.
const W: usize = 5;
/// Corner candidates turn at least this much (degrees).
const CORNER_FLOOR_DEG: f32 = 20.0;
/// A polygon's exterior angles must each be at least this (degrees)…
const CORNER_MIN_ANGLE_DEG: f32 = 22.0;
/// …and at least this fraction of the average exterior angle 360°/m.
const CORNER_REL: f32 = 0.5;
/// Per-parameter cost in the polygon-vs-ellipse model selection (in units
/// of N·ln(residual²)); higher favours blobs.
const BIC_PENALTY: f32 = 7.0;
/// Points the polygon and ellipse residuals are measured on.
const FIT_N: usize = 128;
/// Blob shape: aspect beyond this is Wide/Tall.
const ASPECT: f32 = 1.35;
/// Blob shape: area / convex hull area below this is Bean (if not Wide/Tall).
const BEAN_CONVEXITY: f32 = 0.9;

// ---------------------------------------------------------------------------
// names

/// The shorthand's name for an object an arrow can attach to:
/// Node/Element/Blob → its name, CayleyVertex(c, w) → "c.w",
/// NGonVertex(p, k) → "p.k". Other refs → None.
pub fn obj_name(o: &ObjRef) -> Option<String> {
    match o {
        ObjRef::Node(n) | ObjRef::Element(n) | ObjRef::Blob(n) => Some(n.clone()),
        ObjRef::CayleyVertex(c, w) => Some(format!("{c}.{w}")),
        ObjRef::NGonVertex(p, k) => Some(format!("{p}.{k}")),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// small geometry

fn pt(x: f32, y: f32) -> Pt {
    Pt { x, y }
}
fn sub(a: Pt, b: Pt) -> Pt {
    pt(a.x - b.x, a.y - b.y)
}
fn dist(a: Pt, b: Pt) -> f32 {
    (a.x - b.x).hypot(a.y - b.y)
}
fn lerp(a: Pt, b: Pt, t: f32) -> Pt {
    pt(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}
fn cross(a: Pt, b: Pt) -> f32 {
    a.x * b.y - a.y * b.x
}

fn dist_to_seg(p: Pt, a: Pt, b: Pt) -> f32 {
    let ab = sub(b, a);
    let l2 = ab.x * ab.x + ab.y * ab.y;
    if l2 <= 1e-12 {
        return dist(p, a);
    }
    let t = (((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / l2).clamp(0.0, 1.0);
    dist(p, lerp(a, b, t))
}

fn bbox(pts: &[Pt]) -> Rect {
    let mut r = Rect { min: pt(f32::INFINITY, f32::INFINITY), max: pt(f32::NEG_INFINITY, f32::NEG_INFINITY) };
    for p in pts {
        r.min.x = r.min.x.min(p.x);
        r.min.y = r.min.y.min(p.y);
        r.max.x = r.max.x.max(p.x);
        r.max.y = r.max.y.max(p.y);
    }
    r
}
fn rect_wh(r: &Rect) -> (f32, f32) {
    ((r.max.x - r.min.x).max(0.0), (r.max.y - r.min.y).max(0.0))
}
fn rect_diag(r: &Rect) -> f32 {
    let (w, h) = rect_wh(r);
    w.hypot(h)
}
fn rect_area(r: &Rect) -> f32 {
    let (w, h) = rect_wh(r);
    w * h
}
fn rect_center(r: &Rect) -> Pt {
    pt((r.min.x + r.max.x) / 2.0, (r.min.y + r.max.y) / 2.0)
}
/// Distance from p to the rectangle (0 inside).
fn rect_dist(r: &Rect, p: Pt) -> f32 {
    let dx = (r.min.x - p.x).max(0.0).max(p.x - r.max.x);
    let dy = (r.min.y - p.y).max(0.0).max(p.y - r.max.y);
    dx.hypot(dy)
}

fn path_len(pts: &[Pt]) -> f32 {
    pts.windows(2).map(|w| dist(w[0], w[1])).sum()
}

/// Signed area (counter-clockwise positive) of a closed polygon.
fn signed_area(poly: &[Pt]) -> f32 {
    let n = poly.len();
    if n < 3 {
        return 0.0;
    }
    (0..n).map(|i| cross(poly[i], poly[(i + 1) % n])).sum::<f32>() / 2.0
}

/// Even-odd point-in-polygon.
fn inside(poly: &[Pt], p: Pt) -> bool {
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut c = false;
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            c = !c;
        }
        j = i;
    }
    c
}

/// Distance from p to a polyline (closed: includes the closing edge).
fn dist_to_polyline(poly: &[Pt], closed: bool, p: Pt) -> f32 {
    match poly.len() {
        0 => f32::INFINITY,
        1 => dist(p, poly[0]),
        n => {
            let mut d = poly.windows(2).map(|w| dist_to_seg(p, w[0], w[1])).fold(f32::INFINITY, f32::min);
            if closed {
                d = d.min(dist_to_seg(p, poly[n - 1], poly[0]));
            }
            d
        }
    }
}

/// A Scene path as a polyline (cubics in 16 steps).
fn flatten(start: Pt, segs: &[Seg]) -> Vec<Pt> {
    let mut out = vec![start];
    let mut cur = start;
    for s in segs {
        match *s {
            Seg::Line(p) => {
                out.push(p);
                cur = p;
            }
            Seg::Cubic(c1, c2, e) => {
                for i in 1..=16 {
                    let t = i as f32 / 16.0;
                    let u = 1.0 - t;
                    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                    out.push(pt(
                        a * cur.x + b * c1.x + c * c2.x + d * e.x,
                        a * cur.y + b * c1.y + c * c2.y + d * e.y,
                    ));
                }
                cur = e;
            }
        }
    }
    out
}

/// `n` points evenly spaced by arc length along an open polyline.
fn resample_open(pts: &[Pt], n: usize) -> Vec<Pt> {
    let total = path_len(pts);
    if pts.len() < 2 || total <= 1e-9 || n < 2 {
        return vec![pts[0]; n.max(1)];
    }
    let step = total / (n - 1) as f32;
    let mut out = Vec::with_capacity(n);
    out.push(pts[0]);
    let mut i = 0;
    let mut acc = 0.0; // arc length at pts[i]
    for k in 1..n - 1 {
        let target = step * k as f32;
        while i + 1 < pts.len() - 1 && acc + dist(pts[i], pts[i + 1]) < target {
            acc += dist(pts[i], pts[i + 1]);
            i += 1;
        }
        let l = dist(pts[i], pts[i + 1]);
        let t = if l > 1e-9 { ((target - acc) / l).clamp(0.0, 1.0) } else { 0.0 };
        out.push(lerp(pts[i], pts[i + 1], t));
    }
    out.push(*pts.last().unwrap());
    out
}

/// `n` points evenly spaced along a closed polygon (closing edge included).
fn resample_closed(pts: &[Pt], n: usize) -> Vec<Pt> {
    let mut c = pts.to_vec();
    c.push(pts[0]);
    let mut r = resample_open(&c, n + 1);
    r.pop();
    r
}

/// Cyclic [1 2 1]/4 smoothing, `passes` times.
fn smooth_closed(pts: &[Pt], passes: usize) -> Vec<Pt> {
    let n = pts.len();
    let mut cur = pts.to_vec();
    for _ in 0..passes {
        cur = (0..n)
            .map(|i| {
                let (a, b, c) = (cur[(i + n - 1) % n], cur[i], cur[(i + 1) % n]);
                pt((a.x + 2.0 * b.x + c.x) / 4.0, (a.y + 2.0 * b.y + c.y) / 4.0)
            })
            .collect();
    }
    cur
}

fn wrap_angle(a: f32) -> f32 {
    let mut a = a;
    while a > std::f32::consts::PI {
        a -= std::f32::consts::TAU;
    }
    while a < -std::f32::consts::PI {
        a += std::f32::consts::TAU;
    }
    a
}

/// Convex hull area (monotone chain).
fn hull_area(pts: &[Pt]) -> f32 {
    let mut p: Vec<Pt> = pts.to_vec();
    p.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap().then(a.y.partial_cmp(&b.y).unwrap()));
    if p.len() < 3 {
        return 0.0;
    }
    let mut h: Vec<Pt> = Vec::new();
    for pass in 0..2 {
        let start = h.len();
        let it: Box<dyn Iterator<Item = &Pt>> = if pass == 0 { Box::new(p.iter()) } else { Box::new(p.iter().rev()) };
        for &q in it {
            while h.len() >= start + 2 && cross(sub(h[h.len() - 1], h[h.len() - 2]), sub(q, h[h.len() - 2])) <= 0.0 {
                h.pop();
            }
            h.push(q);
        }
        h.pop();
    }
    signed_area(&h).abs()
}

fn cell_of(scene: &Scene) -> (f32, f32) {
    let (w, h) = scene.cell;
    if w > 0.0 && h > 0.0 { (w, h) } else { DEFAULT_SPACING }
}

// ---------------------------------------------------------------------------
// hit

fn is_point_obj(o: &ObjRef) -> bool {
    matches!(o, ObjRef::Node(_) | ObjRef::Element(_) | ObjRef::CayleyVertex(..) | ObjRef::NGonVertex(..) | ObjRef::Text(_))
}

/// Closed outlines in the scene (blobs, n-gons, any closed path), flattened.
fn closed_shapes(scene: &Scene) -> Vec<(ObjRef, Vec<Pt>)> {
    let mut out: Vec<(ObjRef, Vec<Pt>)> = Vec::new();
    for d in &scene.draws {
        if let Draw::Path { obj, start, segs, closed, .. } = d {
            if *closed || matches!(obj, ObjRef::Blob(_) | ObjRef::NGon(_)) {
                out.push((obj.clone(), flatten(*start, segs)));
            }
        }
    }
    // Blobs / n-gons that have an anchor but no outline: use their box.
    for (o, _, r) in &scene.anchors {
        if matches!(o, ObjRef::Blob(_) | ObjRef::NGon(_)) && !out.iter().any(|(q, _)| q == o) {
            out.push((o.clone(), vec![r.min, pt(r.max.x, r.min.y), r.max, pt(r.min.x, r.max.y)]));
        }
    }
    out
}

/// Which object is under this point (topmost; labels/nodes before paths
/// before blobs). Ties: smallest box (then nearest centre) among
/// labels/nodes; nearest curve among paths; smallest area among closed
/// shapes, so a child blob beats its parent.
pub fn hit(scene: &Scene, at: Pt) -> Option<ObjRef> {
    // 1. nodes / elements / vertices / texts (anchor boxes), dots, labels
    let mut best: Option<(f32, f32, ObjRef)> = None; // (area, centre dist, obj)
    let consider = |o: &ObjRef, c: Pt, r: &Rect, best: &mut Option<(f32, f32, ObjRef)>| {
        if rect_dist(r, at) <= HIT_PAD {
            let key = (rect_area(r), dist(c, at));
            let better = match best {
                None => true,
                Some((a, d, _)) => key.0 < *a - 1e-6 || ((key.0 - *a).abs() <= 1e-6 && key.1 < *d),
            };
            if better {
                *best = Some((key.0, key.1, o.clone()));
            }
        }
    };
    for (o, c, r) in &scene.anchors {
        if is_point_obj(o) {
            consider(o, *c, r, &mut best);
        }
    }
    for d in &scene.draws {
        match d {
            Draw::Dot { obj, at: c, r, .. } => {
                let rr = Rect { min: pt(c.x - r, c.y - r), max: pt(c.x + r, c.y + r) };
                consider(obj, *c, &rr, &mut best);
            }
            Draw::Label { obj, at: c, latex, size, .. } => {
                // rough box: ~0.6 em per character, 1 em tall (size in pt)
                let em = if *size > 0.0 { size / 28.45 } else { 0.35 };
                let w = (latex.chars().filter(|ch| !"\\{}^_".contains(*ch)).count().max(1) as f32 * 0.6 * em).clamp(0.2, 3.0);
                let h = em.max(0.2);
                let rr = Rect { min: pt(c.x - w / 2.0, c.y - h / 2.0), max: pt(c.x + w / 2.0, c.y + h / 2.0) };
                consider(obj, *c, &rr, &mut best);
            }
            _ => {}
        }
    }
    if let Some((_, _, o)) = best {
        return Some(o);
    }
    // 2. open paths: arrows, Cayley edges
    let mut bestp: Option<(f32, ObjRef)> = None;
    for d in &scene.draws {
        if let Draw::Path { obj, start, segs, closed, .. } = d {
            if *closed || matches!(obj, ObjRef::Blob(_) | ObjRef::NGon(_)) {
                continue;
            }
            let dd = dist_to_polyline(&flatten(*start, segs), false, at);
            if dd < HIT_PATH && bestp.as_ref().is_none_or(|(b, _)| dd < *b) {
                bestp = Some((dd, obj.clone()));
            }
        }
    }
    if let Some((_, o)) = bestp {
        return Some(o);
    }
    // 3. closed shapes: smallest containing one
    smallest_containing(&closed_shapes(scene), at, 0.0, |_| true)
}

/// The smallest-area closed shape containing `at` (or within `pad` of its
/// outline) whose ref passes `keep`.
fn smallest_containing(shapes: &[(ObjRef, Vec<Pt>)], at: Pt, pad: f32, keep: impl Fn(&ObjRef) -> bool) -> Option<ObjRef> {
    let mut best: Option<(f32, ObjRef)> = None;
    for (o, poly) in shapes {
        if !keep(o) {
            continue;
        }
        let on = inside(poly, at) || (pad > 0.0 && dist_to_polyline(poly, true, at) <= pad);
        if on {
            let a = signed_area(poly).abs();
            if best.as_ref().is_none_or(|(b, _)| a < *b) {
                best = Some((a, o.clone()));
            }
        }
    }
    best.map(|(_, o)| o)
}

// ---------------------------------------------------------------------------
// recognize

/// Which nameable object an arrow endpoint lies on: point objects (nodes,
/// elements, vertices) by their padded box — the nearest box, then the
/// smallest — else the smallest blob containing it.
fn endpoint(scene: &Scene, shapes: &[(ObjRef, Vec<Pt>)], p: Pt) -> Option<ObjRef> {
    let mut best: Option<(f32, f32, ObjRef)> = None;
    for (o, c, r) in &scene.anchors {
        if !matches!(o, ObjRef::Node(_) | ObjRef::Element(_) | ObjRef::CayleyVertex(..) | ObjRef::NGonVertex(..)) {
            continue;
        }
        let d = rect_dist(r, p);
        if d > ARROW_PAD {
            continue;
        }
        // Distance to the box, bucketed so near-ties go to the nearer centre.
        let key = (d, dist(*c, p));
        let better = match &best {
            None => true,
            Some((bd, bc, _)) => key.0 < *bd - 0.05 || (key.0 <= *bd + 0.05 && key.1 < *bc),
        };
        if better {
            best = Some((key.0, key.1, o.clone()));
        }
    }
    if let Some((_, _, o)) = best {
        return Some(o);
    }
    smallest_containing(shapes, p, ARROW_BLOB_PAD, |o| matches!(o, ObjRef::Blob(_)))
}

/// The object at a stroke's end: at the raw endpoint, else the first object
/// met walking back up to `ARROW_LOOKBACK` of the length (overshoot).
fn stroke_end_obj(scene: &Scene, shapes: &[(ObjRef, Vec<Pt>)], pts: &[Pt], total: f32) -> Option<ObjRef> {
    let mut walked = 0.0;
    let mut prev = pts[0];
    for (i, &p) in pts.iter().enumerate() {
        if i > 0 {
            walked += dist(prev, p);
        }
        prev = p;
        if walked > ARROW_LOOKBACK * total {
            break;
        }
        if let Some(o) = endpoint(scene, shapes, p) {
            return Some(o);
        }
    }
    None
}

/// Average runs of consecutive points that stay within `delta` of the run's
/// first point. Where the hand moved slowly the trackpad reports many
/// jittery points close together; averaging them denoises and keeps arc
/// length following the curve rather than the jitter.
fn declump(pts: &[Pt], delta: f32) -> Vec<Pt> {
    let mut out = Vec::with_capacity(pts.len());
    let mut i = 0;
    while i < pts.len() {
        let first = pts[i];
        let (mut sx, mut sy, mut c) = (0.0f32, 0.0f32, 0usize);
        while i < pts.len() && (c == 0 || dist(pts[i], first) < delta) {
            sx += pts[i].x;
            sy += pts[i].y;
            c += 1;
            i += 1;
        }
        out.push(pt(sx / c as f32, sy / c as f32));
    }
    out
}

/// `n` evenly spaced samples of a closed loop, each the average of 4 finer
/// samples (a box filter along the arc).
fn box_samples(lp: &[Pt], n: usize) -> Vec<Pt> {
    let fine = resample_closed(lp, 4 * n);
    (0..n)
        .map(|i| {
            let g = &fine[4 * i..4 * i + 4];
            pt(g.iter().map(|p| p.x).sum::<f32>() / 4.0, g.iter().map(|p| p.y).sum::<f32>() / 4.0)
        })
        .collect()
}

/// The loop as `N` evenly spaced, denoised samples (declumped, box
/// filtered, two [1 2 1] passes): for the loop test, corner detection, the
/// blob's box and shape.
fn loop_samples(lp: &[Pt]) -> Vec<Pt> {
    let lp = declump(lp, DECLUMP * rect_diag(&bbox(lp)));
    smooth_closed(&box_samples(&lp, N), 2)
}

/// Area centroid, second central moments of the filled region (sxx, syy,
/// sxy) and signed area of a closed polygon.
fn moments(q: &[Pt]) -> (Pt, (f32, f32, f32), f32) {
    let n = q.len();
    let a = signed_area(q);
    if a.abs() < 1e-12 {
        return (q[0], (0.0, 0.0, 0.0), 0.0);
    }
    let (mut cx, mut cy) = (0.0f32, 0.0f32);
    for i in 0..n {
        let (p, r) = (q[i], q[(i + 1) % n]);
        let c = cross(p, r);
        cx += (p.x + r.x) * c;
        cy += (p.y + r.y) * c;
    }
    let c = pt(cx / (6.0 * a), cy / (6.0 * a));
    let (mut ixx, mut iyy, mut ixy) = (0.0f32, 0.0f32, 0.0f32);
    for i in 0..n {
        let (p, r) = (sub(q[i], c), sub(q[(i + 1) % n], c));
        let k = cross(p, r);
        ixx += k * (p.x * p.x + p.x * r.x + r.x * r.x);
        iyy += k * (p.y * p.y + p.y * r.y + r.y * r.y);
        ixy += k * (p.x * r.y + 2.0 * p.x * p.y + 2.0 * r.x * r.y + r.x * p.y);
    }
    (c, (ixx / 12.0 / a, iyy / 12.0 / a, ixy / 24.0 / a), a)
}

/// RMS distance from `pts` to the moment ellipse of loop `q` (the ellipse
/// with the same area centroid and second moments; exact for an ellipse).
fn ellipse_residual(q: &[Pt], pts: &[Pt]) -> f32 {
    let (c, (sxx, syy, sxy), a) = moments(q);
    if a.abs() < 1e-12 {
        return f32::INFINITY;
    }
    let tr = (sxx + syy) / 2.0;
    let det = ((sxx - syy) / 2.0).hypot(sxy);
    let (l1, l2) = ((tr + det).max(1e-12), (tr - det).max(1e-12));
    let th = 0.5 * (2.0 * sxy).atan2(sxx - syy);
    // a filled ellipse with semi-axes (a, b) has variances a²/4, b²/4
    let (ea, eb) = (2.0 * l1.sqrt(), 2.0 * l2.sqrt());
    let (ct, st) = (th.cos(), th.sin());
    let mut sum = 0.0;
    for p in pts {
        let d = sub(*p, c);
        let (u, v) = (d.x * ct + d.y * st, -d.x * st + d.y * ct);
        let rho = ((u / ea).powi(2) + (v / eb).powi(2)).sqrt().max(1e-6);
        let e = (1.0 - 1.0 / rho).abs() * u.hypot(v); // radial distance
        sum += e * e;
    }
    (sum / pts.len() as f32).sqrt()
}

/// The polygon through the corners `cs` of loop `q`, each corner refined to
/// the intersection of the lines fitted (total least squares) to the middle
/// 70% of its two runs — so rounded or smoothed corners don't pull it in.
fn refined_polygon(q: &[Pt], cs: &[usize]) -> Vec<Pt> {
    let n = q.len();
    let m = cs.len();
    let lines: Vec<(Pt, Pt)> = (0..m)
        .map(|j| {
            let (a, b) = (cs[j], cs[(j + 1) % m]);
            let len = (b + n - a) % n;
            let skip = (len as f32 * 0.15).round() as usize;
            let idx: Vec<usize> = (skip..=len.saturating_sub(skip)).map(|o| (a + o) % n).collect();
            let idx = if idx.len() >= 2 { idx } else { vec![a, b] };
            let c = idx.iter().fold(pt(0.0, 0.0), |s, &i| pt(s.x + q[i].x, s.y + q[i].y));
            let c = pt(c.x / idx.len() as f32, c.y / idx.len() as f32);
            let (mut sxx, mut syy, mut sxy) = (0.0f32, 0.0f32, 0.0f32);
            for &i in &idx {
                let d = sub(q[i], c);
                sxx += d.x * d.x;
                syy += d.y * d.y;
                sxy += d.x * d.y;
            }
            let th = 0.5 * (2.0 * sxy).atan2(sxx - syy);
            (c, pt(th.cos(), th.sin()))
        })
        .collect();
    (0..m)
        .map(|j| {
            let (p1, d1) = lines[(j + m - 1) % m];
            let (p2, d2) = lines[j];
            let den = cross(d1, d2);
            let here = q[cs[j]];
            if den.abs() < 0.1 {
                return here;
            }
            let t = cross(sub(p2, p1), d2) / den;
            let x = pt(p1.x + d1.x * t, p1.y + d1.y * t);
            // a wild intersection (nearly parallel runs): keep the sample
            let near = dist(here, q[cs[(j + 1) % m]]).max(dist(here, q[cs[(j + m - 1) % m]]));
            if dist(x, here) > 0.5 * near { here } else { x }
        })
        .collect()
}

/// RMS distance from a closed loop to a closed polygon.
fn polygon_residual(q: &[Pt], poly: &[Pt]) -> f32 {
    let sum: f32 = q.iter().map(|p| dist_to_polyline(poly, true, *p).powi(2)).sum();
    (sum / q.len() as f32).sqrt()
}

/// The corners of a closed loop, if it is a polygon (3–9 corners, refined
/// by `refined_polygon`), else None (a blob).
///
/// `q` is the denoised, evenly sampled loop (`loop_samples`); `fit` is the
/// loop declumped but not smoothed (`FIT_N` points), where the residuals are
/// measured so that corners stay sharp.
///
/// Corner detection on the turning angle: the strength of sample i is the
/// turn between the chords from the sample `W` before to i and from i to
/// the sample `W` after (long chords average the trackpad's jitter out). A
/// smooth round loop turns evenly, 360°·W/N ≈ 28° per chord pair; a
/// hexagon's corner turns ≈ 60°, a triangle's 120°. The candidates are the
/// local peaks (±3 samples) of at least `CORNER_FLOOR_DEG`, strongest first.
///
/// Then model selection (a BIC): for m = 3..9, the polygon on the m
/// strongest candidates competes with the loop's moment ellipse on
/// `FIT_N`·ln(RMS residual²) + `BIC_PENALTY`·(parameters) — 2m for the
/// polygon, 5 for the ellipse; the least wins. So a weak corner still counts when it
/// makes the polygon fit much better, and a noisy circle's spurious peaks
/// never buy enough fit to pay for themselves. A polygon is only a candidate
/// when each exterior angle is a real turn: at least `CORNER_MIN_ANGLE_DEG`
/// and `CORNER_REL` × the average 360°/m.
fn corners(q: &[Pt], fit: &[Pt]) -> Option<Vec<Pt>> {
    let n = q.len();
    let orient = if signed_area(q) >= 0.0 { 1.0 } else { -1.0 };
    let turn = |a: Pt, b: Pt| orient * wrap_angle(b.y.atan2(b.x) - a.y.atan2(a.x));
    let k: Vec<f32> = (0..n).map(|i| turn(sub(q[i], q[(i + n - W) % n]), sub(q[(i + W) % n], q[i]))).collect();
    let floor = CORNER_FLOOR_DEG.to_radians();
    let mut peaks: Vec<usize> = (0..n)
        .filter(|&i| {
            k[i] >= floor
                // a local maximum over ±3 (ties: the first of a plateau)
                && (1..=3).all(|d| k[i] >= k[(i + d) % n] && (k[i] > k[(i + n - d) % n] || (k[i] == k[(i + n - d) % n] && d > 1)))
        })
        .collect();
    peaks.sort_by(|&a, &b| k[b].total_cmp(&k[a]));
    let ff = fit.len() as f32;
    let re = ellipse_residual(q, fit).max(1e-6);
    let mut best = (ff * (re * re).ln() + BIC_PENALTY * 5.0, None);
    // the polygon on the m sharpest turns, m = 3..9
    for m in 3..=9usize.min(peaks.len()) {
        let mut cs: Vec<usize> = peaks[..m].to_vec();
        cs.sort_unstable();
        let poly = refined_polygon(q, &cs);
        let min_turn = CORNER_MIN_ANGLE_DEG.max(CORNER_REL * 360.0 / m as f32).to_radians();
        let real = (0..m).all(|j| turn(sub(poly[j], poly[(j + m - 1) % m]), sub(poly[(j + 1) % m], poly[j])) >= min_turn);
        if !real {
            continue;
        }
        let rp = polygon_residual(fit, &poly).max(1e-6);
        let cost = ff * (rp * rp).ln() + BIC_PENALTY * 2.0 * m as f32;
        if cost < best.0 {
            best = (cost, Some(poly));
        }
    }
    best.1
}

/// A closed loop: the stroke trimmed of overshoot, if it closes.
fn closed_loop(pts: &[Pt]) -> Option<Vec<Pt>> {
    if pts.len() < 5 {
        return None;
    }
    let total = path_len(pts);
    // arc length at each raw point
    let mut s = Vec::with_capacity(pts.len());
    let mut acc = 0.0;
    for (i, p) in pts.iter().enumerate() {
        if i > 0 {
            acc += dist(pts[i - 1], *p);
        }
        s.push(acc);
    }
    // closest raw pair between the first and last quarter (trims overshoot)
    let (mut bi, mut bj, mut bd) = (0usize, pts.len() - 1, f32::INFINITY);
    for i in 0..pts.len() {
        if s[i] > 0.25 * total {
            break;
        }
        for j in (0..pts.len()).rev() {
            if s[j] < 0.75 * total {
                break;
            }
            let d = dist(pts[i], pts[j]);
            if d < bd - 1e-6 || (d <= bd + 1e-6 && j - i > bj - bi) {
                (bi, bj, bd) = (i, j, d);
            }
        }
    }
    let mut lp: Vec<Pt> = pts[bi..=bj].to_vec();
    if lp.len() < 4 {
        return None;
    }
    let diag = rect_diag(&bbox(&lp));
    if diag < TAP || bd > LOOP_GAP * diag {
        return None;
    }
    // A gap that cuts a corner: carry both ends on along their tangents and,
    // if they meet close by, put the corner back (else the chord bridges it).
    if bd > 0.02 * diag {
        let reach = bd.max(0.05 * path_len(&lp));
        let back = |seq: &mut dyn Iterator<Item = Pt>| -> Pt {
            let first = seq.next().unwrap();
            let mut last = first;
            for p in seq {
                last = p;
                if dist(p, first) >= reach {
                    break;
                }
            }
            last
        };
        let e = *lp.last().unwrap();
        let s0 = lp[0];
        let e_from = back(&mut lp.iter().rev().copied());
        let s_to = back(&mut lp.iter().copied());
        let (de, ds) = (sub(e, e_from), sub(s0, s_to)); // both point into the gap
        let den = cross(de, ds);
        if den.abs() > 1e-9 {
            // e + t·de = s0 + u·ds
            let w = sub(s0, e);
            let t = cross(w, ds) / den;
            let u = cross(w, de) / den;
            let x = pt(e.x + de.x * t, e.y + de.y * t);
            if t > 0.0 && u > 0.0 && dist(x, e) < 1.5 * bd && dist(x, s0) < 1.5 * bd {
                lp.push(x);
            }
        }
    }
    let q = loop_samples(&lp);
    let c = q.iter().fold(pt(0.0, 0.0), |a, p| pt(a.x + p.x / q.len() as f32, a.y + p.y / q.len() as f32));
    let turn: f32 = (0..q.len())
        .map(|i| {
            let (a, b) = (sub(q[i], c), sub(q[(i + 1) % q.len()], c));
            wrap_angle(b.y.atan2(b.x) - a.y.atan2(a.x))
        })
        .sum();
    if turn.abs() < LOOP_TURN_DEG.to_radians() {
        return None;
    }
    let per: f32 = path_len(&q) + dist(q[q.len() - 1], q[0]);
    let iso = 4.0 * std::f32::consts::PI * signed_area(&q).abs() / (per * per).max(1e-9);
    if iso < LOOP_ISO {
        return None;
    }
    Some(lp)
}

/// What a rough canvas gesture is (see the module doc).
pub fn recognize(stroke: &[Pt], scene: &Scene) -> Recognized {
    let pts: Vec<Pt> = stroke.iter().copied().filter(|p| p.x.is_finite() && p.y.is_finite()).collect();
    if pts.is_empty() {
        return Recognized::Nothing;
    }
    let total = path_len(&pts);
    let bb = bbox(&pts);
    // 1. tap
    if pts.len() == 1 || (total < TAP && rect_diag(&bb) < TAP) {
        let c = pts.iter().fold(pt(0.0, 0.0), |a, p| pt(a.x + p.x, a.y + p.y));
        let c = pt(c.x / pts.len() as f32, c.y / pts.len() as f32);
        let (cw, ch) = cell_of(scene);
        let col = (c.x / cw).round() + 0.0;
        let row = (-c.y / ch).round() + 0.0; // + 0.0 turns -0 into 0
        return Recognized::Node(Pos::Cell(col, row));
    }
    // 2. closed loop → n-gon or blob
    if let Some(lp) = closed_loop(&pts) {
        let q = loop_samples(&lp);
        let fit = resample_closed(&declump(&lp, DECLUMP * rect_diag(&bbox(&lp))), FIT_N);
        if let Some(cp) = corners(&q, &fit) {
            let n = cp.len() as u32;
            let m = cp.len() as f32;
            let center = cp.iter().fold(pt(0.0, 0.0), |a, p| pt(a.x + p.x / m, a.y + p.y / m));
            let radius = cp.iter().map(|p| dist(*p, center)).sum::<f32>() / m;
            return Recognized::NGon { n, center, radius };
        }
        let r = bbox(&q);
        let (w, h) = rect_wh(&r);
        let shape = if w > ASPECT * h {
            BlobShape::Wide
        } else if h > ASPECT * w {
            BlobShape::Tall
        } else if signed_area(&q).abs() < BEAN_CONVEXITY * hull_area(&q) {
            BlobShape::Bean
        } else {
            BlobShape::Round
        };
        let mut encloses: Vec<String> = Vec::new();
        for (o, c, _) in &scene.anchors {
            if matches!(o, ObjRef::Node(_) | ObjRef::Element(_) | ObjRef::Blob(_)) && inside(&lp, *c) {
                if let Some(nm) = obj_name(o) {
                    if !encloses.contains(&nm) {
                        encloses.push(nm);
                    }
                }
            }
        }
        return Recognized::Blob { center: rect_center(&r), size: (w, h), shape, encloses };
    }
    // 3. arrow
    let shapes = closed_shapes(scene);
    let rev: Vec<Pt> = pts.iter().rev().copied().collect();
    let from = stroke_end_obj(scene, &shapes, &pts, total);
    let to = stroke_end_obj(scene, &shapes, &rev, total);
    if let (Some(f), Some(t)) = (from, to) {
        if f != t {
            if let (Some(from), Some(to)) = (obj_name(&f), obj_name(&t)) {
                return Recognized::Arrow { from, to };
            }
        }
    }
    Recognized::Nothing
}
