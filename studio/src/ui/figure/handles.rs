//! Direct manipulation, the geometry (FIGURES.md "Direct manipulation"):
//! where each selected object's handles sit, what a drag on one does to the
//! object's item, the live readout, and the selection outlines. Pure
//! functions over the Figure and the Scene; no GPUI state.
//!
//! The shapes' frames come from `Scene::shapes` (FIG-MODEL: centre, radius,
//! half extents, angle). While a scene has none (an older layout), they are
//! worked out here from the anchors and the items.
use super::geom::{self, Xf};
use ntex_figures::layout;
use ntex_figures::model::*;

/// What a handle does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HKind {
    /// n-gon: the dot on the circumcircle sets `radius`.
    Radius,
    /// n-gon, blob, Cayley graph: the ↻ knob sets `angle`.
    Rotate,
    /// blob: one of the 8 handles on its box, (sx, sy) ∈ {−1, 0, 1}²
    /// (not both 0), in the blob's own frame.
    Corner(i8, i8),
    /// Cayley graph: the dot on the circle through its farthest vertex sets `radius`.
    Scale,
    /// arrow: the midpoint handle sets `bend`.
    Bend,
    /// arrow: its label; dragged across the arrow it sets `swap`.
    ArrowLabel,
    /// (figure-qol D) arrow: the dot at one end (false = the start, true =
    /// the head); dropped on another object it reattaches that end.
    ArrowEnd(bool),
    /// (figure-qol D) n-gon rotation arrow: dragged round, it moves to the
    /// nearest edge (`rotation edge=`).
    RotationSlide,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Handle {
    pub kind: HKind,
    /// Where it is drawn (cm).
    pub at: Pt,
    pub obj: ObjRef,
}

/// A shape's frame (cm): from the Scene, or worked out locally.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub centre: Pt,
    pub radius: f32,
    pub half: (f32, f32),
    /// Degrees counter-clockwise, as drawn.
    pub angle: f32,
    /// The part of `angle` inherited from a parent blob.
    pub inherited: f32,
}

/// Handle sizes, px.
pub const HANDLE_PX: f32 = 7.0;
pub const KNOB_PX: f32 = 17.0;
/// How far the ↻ knob sits outside the shape, px.
pub const KNOB_GAP_PX: f32 = 22.0;
/// A press this close to a handle (px) takes the handle.
pub const GRAB_PX: f32 = 12.5;

pub fn dir(deg: f32) -> Pt {
    let r = deg.to_radians();
    Pt { x: r.cos(), y: r.sin() }
}

fn add(a: Pt, b: Pt) -> Pt {
    Pt { x: a.x + b.x, y: a.y + b.y }
}
fn sub(a: Pt, b: Pt) -> Pt {
    Pt { x: a.x - b.x, y: a.y - b.y }
}
fn mul(a: Pt, k: f32) -> Pt {
    Pt { x: a.x * k, y: a.y * k }
}
/// Turn `v` by `deg` counter-clockwise.
pub fn rot(v: Pt, deg: f32) -> Pt {
    let (s, c) = deg.to_radians().sin_cos();
    Pt { x: v.x * c - v.y * s, y: v.x * s + v.y * c }
}

/// The scene's scale (0 = unknown → the figure's).
pub fn scene_scale(scene: &Scene, fig: &Figure) -> f32 {
    if scene.scale > 0.0 {
        scene.scale
    } else if fig.globals.scale > 0.0 {
        fig.globals.scale
    } else {
        1.0
    }
}

/// The item an object lives in (an n-gon's vertex → the n-gon, an element
/// → its blob, a Cayley vertex → the graph).
pub fn item_index(fig: &Figure, obj: &ObjRef) -> Option<usize> {
    fig.items.iter().enumerate().find_map(|(i, it)| {
        let hit = match (it, obj) {
            (Item::Node(n), ObjRef::Node(m)) => &n.name == m,
            (Item::Arrow(_), ObjRef::Arrow(k)) | (Item::Text(_), ObjRef::Text(k)) => i == *k,
            (Item::Blob(b), ObjRef::Blob(m)) => &b.name == m,
            (Item::Blob(b), ObjRef::Element(m)) => b.elements.iter().any(|e| &e.name == m),
            (Item::NGon(g), ObjRef::NGon(m) | ObjRef::NGonVertex(m, _) | ObjRef::NGonRotation(m)) => &g.name == m,
            (Item::Cayley(c), ObjRef::Cayley(m) | ObjRef::CayleyVertex(m, _) | ObjRef::CayleyEdge(m, _)) => &c.name == m,
            _ => false,
        };
        hit.then_some(i)
    })
}

/// The shape's frame: `Scene::shapes`, else from the anchors.
pub fn frame(fig: &Figure, scene: &Scene, obj: &ObjRef) -> Option<Frame> {
    // the rotation arrow is a part: its frame is its polygon's
    if let ObjRef::NGonRotation(n) = obj {
        return frame(fig, scene, &ObjRef::NGon(n.clone()));
    }
    if let Some(s) = scene.shapes.iter().find(|s| &s.obj == obj) {
        return Some(Frame { centre: s.centre, radius: s.radius, half: s.half, angle: s.angle, inherited: s.inherited });
    }
    let scale = scene_scale(scene, fig);
    let (centre, rect) = scene.anchors.iter().find(|(o, _, _)| o == obj).map(|(_, p, r)| (*p, r.clone()))?;
    let it = &fig.items[item_index(fig, obj)?];
    match it {
        Item::NGon(g) => {
            let r = g.radius.unwrap_or(layout::NGON_RADIUS) * scale;
            Some(Frame { centre, radius: r, half: (r, r), angle: g.angle, inherited: 0.0 })
        }
        Item::Blob(b) => {
            // the drawn outline, turned back into the blob's frame
            let mut half = (0.0f32, 0.0f32);
            for d in &scene.draws {
                if let Draw::Path { obj: o, start, segs, closed: true, .. } = d
                    && o == obj
                {
                    for q in geom::flatten(*start, segs) {
                        let l = rot(sub(q, centre), -b.angle);
                        half.0 = half.0.max(l.x.abs());
                        half.1 = half.1.max(l.y.abs());
                    }
                }
            }
            if half.0 <= 0.0 {
                half = ((rect.max.x - rect.min.x) / 2.0, (rect.max.y - rect.min.y) / 2.0);
            }
            // the outline wanders past the semi-axes; the size= the box means
            let half = match b.size {
                Some((w, h)) => (w * scale / 2.0, h * scale / 2.0),
                None => (half.0 / 1.03, half.1 / 1.03),
            };
            Some(Frame { centre, radius: half.0.hypot(half.1), half, angle: b.angle, inherited: 0.0 })
        }
        Item::Cayley(c) => {
            let mut far = 0.0f32;
            let mut half = (0.0f32, 0.0f32);
            for (o, p, _) in &scene.anchors {
                if let ObjRef::CayleyVertex(n, _) = o
                    && n == &c.name
                {
                    far = far.max(geom::dist(*p, centre));
                    let l = rot(sub(*p, centre), -c.angle);
                    half.0 = half.0.max(l.x.abs());
                    half.1 = half.1.max(l.y.abs());
                }
            }
            Some(Frame { centre, radius: far.max(0.3), half, angle: c.angle, inherited: 0.0 })
        }
        _ => None,
    }
}

/// The box around everything an object draws (cm): its paths, dots,
/// labels and anchor, and for an n-gon or a Cayley graph its vertices.
pub fn outline(scene: &Scene, obj: &ObjRef) -> Option<Rect> {
    let belongs = |o: &ObjRef| -> bool {
        if o == obj {
            return true;
        }
        match (obj, o) {
            (ObjRef::NGon(n), ObjRef::NGonVertex(m, _) | ObjRef::NGonRotation(m)) => n == m,
            (ObjRef::Cayley(n), ObjRef::CayleyVertex(m, _) | ObjRef::CayleyEdge(m, _)) => n == m,
            _ => false,
        }
    };
    let mut r: Option<Rect> = None;
    fn grow_into(r: &mut Option<Rect>, q: Rect) {
        *r = Some(match r.take() {
            None => q,
            Some(a) => Rect {
                min: Pt { x: a.min.x.min(q.min.x), y: a.min.y.min(q.min.y) },
                max: Pt { x: a.max.x.max(q.max.x), y: a.max.y.max(q.max.y) },
            },
        });
    }
    // a node's or an element's box is its anchor; a vertex label its own box
    let point_like = matches!(obj, ObjRef::Node(_) | ObjRef::Element(_) | ObjRef::CayleyVertex(..));
    for (o, _, rr) in &scene.anchors {
        if o == obj && point_like {
            grow_into(&mut r, rr.clone());
        }
    }
    if point_like && r.is_some() {
        return r;
    }
    if let ObjRef::NGonVertex(..) = obj {
        // just the label (and the vertex dot)
        for d in &scene.draws {
            match d {
                Draw::Label { obj: o, at, latex, size, math, .. } if o == obj => grow_into(&mut r, label_rect(*at, latex, *size, *math)),
                _ => {}
            }
        }
        if r.is_none() {
            for d in &scene.draws {
                if let Draw::Dot { obj: o, at, .. } = d
                    && o == obj
                {
                    grow_into(&mut r, Rect { min: Pt { x: at.x - 0.1, y: at.y - 0.1 }, max: Pt { x: at.x + 0.1, y: at.y + 0.1 } });
                }
            }
        }
        return r;
    }
    for d in &scene.draws {
        match d {
            Draw::Path { obj: o, start, segs, .. } if belongs(o) => grow_into(&mut r, geom::bbox(&geom::flatten(*start, segs))),
            Draw::Dot { obj: o, at, r: rr, .. } if belongs(o) => {
                let rr = if *rr > 0.5 { rr / geom::PT_PER_CM } else { *rr };
                grow_into(&mut r, Rect { min: Pt { x: at.x - rr, y: at.y - rr }, max: Pt { x: at.x + rr, y: at.y + rr } });
            }
            Draw::Label { obj: o, at, latex, size, math, .. } if belongs(o) => grow_into(&mut r, label_rect(*at, latex, *size, *math)),
            _ => {}
        }
    }
    if r.is_none() {
        return geom::extent_of(scene, obj);
    }
    r
}

/// (figure-qol A) A turned shape's outline: the box round everything it draws,
/// measured in the shape's own (turned) frame, so the selection box turns with it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Turned {
    pub centre: Pt,
    /// Half width and height, in the turned frame.
    pub half: Pt,
    /// Degrees counter-clockwise.
    pub angle: f32,
}

impl Turned {
    /// The four corners, grown by `grow` (cm) all round, counter-clockwise from the lower left.
    pub fn corners(&self, grow: f32) -> [Pt; 4] {
        let (hx, hy) = (self.half.x + grow, self.half.y + grow);
        [(-hx, -hy), (hx, -hy), (hx, hy), (-hx, hy)].map(|(x, y)| add(self.centre, rot(Pt { x, y }, self.angle)))
    }
}

/// The turned outline of a shape that is turned (an n-gon, blob or Cayley
/// graph at an angle); None for anything else, which keeps its upright box.
pub fn turned_outline(scene: &Scene, obj: &ObjRef) -> Option<Turned> {
    let f = scene.shapes.iter().find(|s| &s.obj == obj)?;
    if f.angle.rem_euclid(360.0).min(360.0 - f.angle.rem_euclid(360.0)) < 0.5 {
        return None;
    }
    let belongs = |o: &ObjRef| -> bool {
        o == obj
            || match (obj, o) {
                (ObjRef::NGon(n), ObjRef::NGonVertex(m, _) | ObjRef::NGonRotation(m)) => n == m,
                (ObjRef::Cayley(n), ObjRef::CayleyVertex(m, _) | ObjRef::CayleyEdge(m, _)) => n == m,
                _ => false,
            }
    };
    let mut pts: Vec<Pt> = Vec::new();
    for d in &scene.draws {
        match d {
            Draw::Path { obj: o, start, segs, .. } if belongs(o) => pts.extend(geom::flatten(*start, segs)),
            Draw::Dot { obj: o, at, r, .. } if belongs(o) => {
                let rr = if *r > 0.5 { r / geom::PT_PER_CM } else { *r };
                pts.extend([Pt { x: at.x - rr, y: at.y - rr }, Pt { x: at.x + rr, y: at.y + rr }]);
            }
            Draw::Label { obj: o, at, latex, size, math, .. } if belongs(o) => {
                let r = label_rect(*at, latex, *size, *math);
                pts.extend([r.min, r.max, Pt { x: r.min.x, y: r.max.y }, Pt { x: r.max.x, y: r.min.y }]);
            }
            _ => {}
        }
    }
    if pts.is_empty() {
        return None;
    }
    let (mut lo, mut hi) = (Pt { x: f32::MAX, y: f32::MAX }, Pt { x: f32::MIN, y: f32::MIN });
    for q in pts {
        let l = rot(sub(q, f.centre), -f.angle);
        lo = Pt { x: lo.x.min(l.x), y: lo.y.min(l.y) };
        hi = Pt { x: hi.x.max(l.x), y: hi.y.max(l.y) };
    }
    let mid = Pt { x: (lo.x + hi.x) / 2.0, y: (lo.y + hi.y) / 2.0 };
    Some(Turned { centre: add(f.centre, rot(mid, f.angle)), half: Pt { x: (hi.x - lo.x) / 2.0, y: (hi.y - lo.y) / 2.0 }, angle: f.angle })
}

/// A label's box (cm), centred on `at`.
pub fn label_rect(at: Pt, latex: &str, size: f32, math: bool) -> Rect {
    let size = if size > 0.0 { size } else { 10.0 };
    let (hw, hh) = layout::label_half(latex, size, math, 0.0);
    let (hw, hh) = (hw.max(0.08), hh.max(0.1));
    Rect { min: Pt { x: at.x - hw, y: at.y - hh }, max: Pt { x: at.x + hw, y: at.y + hh } }
}

/// The handles of one selected object. `px_per_cm` places the knob a fixed
/// number of pixels outside the shape.
pub fn handles(fig: &Figure, scene: &Scene, obj: &ObjRef, px_per_cm: f32) -> Vec<Handle> {
    let gap = KNOB_GAP_PX / px_per_cm.max(1.0);
    let mut out = Vec::new();
    match obj {
        ObjRef::NGon(_) => {
            let Some(f) = frame(fig, scene, obj) else { return out };
            let n = match item_index(fig, obj).map(|i| &fig.items[i]) {
                Some(Item::NGon(g)) => g.n.max(3),
                _ => 4,
            };
            // the resize dot: on the circumcircle, outside the edge nearest
            // the lower right (clear of the vertex labels)
            let mid = (1..=n)
                .map(|k| layout::ngon_vertex_angle(n, k) + f.angle + 180.0 / n as f32)
                .min_by(|a, b| ang_dist(*a, -30.0).total_cmp(&ang_dist(*b, -30.0)))
                .unwrap_or(0.0);
            out.push(Handle { kind: HKind::Radius, at: add(f.centre, mul(dir(mid), f.radius)), obj: obj.clone() });
            // the knob: above the shape (its "up", so it turns with it),
            // past the vertex labels
            let reach = outline(scene, obj).map(|r| reach_from(&r, f.centre)).unwrap_or(f.radius + 0.45).max(f.radius + 0.3);
            out.push(Handle { kind: HKind::Rotate, at: add(f.centre, mul(dir(90.0 + f.angle), reach + gap)), obj: obj.clone() });
        }
        ObjRef::Blob(_) => {
            let Some(f) = frame(fig, scene, obj) else { return out };
            for sy in [1i8, 0, -1] {
                for sx in [-1i8, 0, 1] {
                    if sx == 0 && sy == 0 {
                        continue;
                    }
                    let l = Pt { x: sx as f32 * f.half.0, y: sy as f32 * f.half.1 };
                    out.push(Handle { kind: HKind::Corner(sx, sy), at: add(f.centre, rot(l, f.angle)), obj: obj.clone() });
                }
            }
            out.push(Handle { kind: HKind::Rotate, at: add(f.centre, rot(Pt { x: 0.0, y: f.half.1 + gap }, f.angle)), obj: obj.clone() });
        }
        ObjRef::Cayley(_) => {
            let Some(f) = frame(fig, scene, obj) else { return out };
            let reach = outline(scene, obj).map(|r| reach_from(&r, f.centre)).unwrap_or(f.radius + 0.3);
            // the resize dot: the lower-right corner of its box (clear of the
            // vertices); only where a set radius= keeps the drawing
            let resizable = match item_index(fig, obj).map(|i| &fig.items[i]) {
                Some(Item::Cayley(c)) => cayley_resizable(c),
                _ => false,
            };
            let at = outline(scene, obj)
                .map(|r| Pt { x: r.max.x + 0.5 * gap, y: r.min.y - 0.5 * gap })
                .unwrap_or_else(|| add(f.centre, mul(dir(-45.0), f.radius + gap)));
            if resizable {
                out.push(Handle { kind: HKind::Scale, at, obj: obj.clone() });
            }
            out.push(Handle { kind: HKind::Rotate, at: add(f.centre, mul(dir(90.0 + f.angle), reach + gap)), obj: obj.clone() });
        }
        ObjRef::Arrow(_) => {
            let label = scene.draws.iter().find_map(|d| match d {
                Draw::Label { obj: o, at, .. } if o == obj => Some(*at),
                _ => None,
            });
            // the bend handle on the arrow, at least 28 px from its label's handle
            let far = 28.0 / px_per_cm.max(1.0);
            let pts = scene.draws.iter().find_map(|d| match d {
                Draw::Path { obj: o, start, segs, .. } if o == obj => Some(geom::flatten(*start, segs)),
                _ => None,
            });
            if let Some(pts) = pts.filter(|p| p.len() >= 2) {
                let at_t = |t: f32| -> Pt {
                    let total: f32 = pts.windows(2).map(|w| geom::dist(w[0], w[1])).sum();
                    let want = total * t;
                    let mut acc = 0.0;
                    for w in pts.windows(2) {
                        let l = geom::dist(w[0], w[1]);
                        if acc + l >= want && l > 0.0 {
                            let u = (want - acc) / l;
                            return Pt { x: w[0].x + u * (w[1].x - w[0].x), y: w[0].y + u * (w[1].y - w[0].y) };
                        }
                        acc += l;
                    }
                    *pts.last().unwrap()
                };
                let mut at = at_t(0.5);
                if let Some(l) = label {
                    for t in [0.5f32, 0.42, 0.58, 0.35, 0.65, 0.28, 0.72] {
                        let q = at_t(t);
                        at = q;
                        if geom::dist(q, l) >= far {
                            break;
                        }
                    }
                }
                out.push(Handle { kind: HKind::Bend, at, obj: obj.clone() });
            }
            if let Some(at) = label {
                out.push(Handle { kind: HKind::ArrowLabel, at, obj: obj.clone() });
            }
            // (figure-qol D) a dot at each end: drag one onto another object
            let ends = scene.draws.iter().find_map(|d| match d {
                Draw::Path { obj: o, start, segs, .. } if o == obj => {
                    let pts = geom::flatten(*start, segs);
                    Some((*pts.first()?, *pts.last()?))
                }
                _ => None,
            });
            if let Some((a, b)) = ends {
                out.push(Handle { kind: HKind::ArrowEnd(false), at: a, obj: obj.clone() });
                out.push(Handle { kind: HKind::ArrowEnd(true), at: b, obj: obj.clone() });
            }
        }
        ObjRef::NGonRotation(_) => {
            // a grip at the middle of the arc
            let pts = scene.draws.iter().find_map(|d| match d {
                Draw::Path { obj: o, start, segs, .. } if o == obj => Some(geom::flatten(*start, segs)),
                _ => None,
            });
            if let Some(pts) = pts.filter(|p| p.len() >= 2) {
                out.push(Handle { kind: HKind::RotationSlide, at: pts[pts.len() / 2], obj: obj.clone() });
            }
        }
        _ => {}
    }
    out
}

/// How far the corners of `r` reach from `c` straight up (the knob goes past it).
fn reach_from(r: &Rect, c: Pt) -> f32 {
    // the farthest corner: outside the shape whichever way it is turned
    [Pt { x: r.min.x, y: r.min.y }, Pt { x: r.max.x, y: r.min.y }, Pt { x: r.min.x, y: r.max.y }, Pt { x: r.max.x, y: r.max.y }]
        .iter()
        .map(|q| geom::dist(c, *q))
        .fold(0.0, f32::max)
}

fn ang_dist(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

/// The handle under a point (window px), nearest first.
pub fn hit(handles: &[Handle], at: gpui::Point<gpui::Pixels>, xf: &Xf) -> Option<Handle> {
    let mut best: Option<(f32, &Handle)> = None;
    for h in handles {
        let q = xf.to_px(h.at);
        let d = (f32::from(q.x - at.x)).hypot(f32::from(q.y - at.y));
        let reach = match h.kind {
            HKind::Rotate => KNOB_PX / 2.0 + 4.0,
            HKind::ArrowLabel => 13.0,
            _ => GRAB_PX,
        };
        if d <= reach && best.is_none_or(|(b, _)| d < b) {
            best = Some((d, h));
        }
    }
    best.map(|(_, h)| h.clone())
}

// ---------------------------------------------------------------------------
// dragging a handle

/// Everything a handle drag needs from its start.
#[derive(Clone, Debug)]
pub struct DragStart {
    pub handle: Handle,
    pub index: usize,
    pub item: Item,
    pub frame: Option<Frame>,
    /// Where the press was (cm).
    pub grab: Pt,
    pub scale: f32,
    /// A top-level blob may move as it is resized (the opposite side stays put).
    pub top_level: bool,
    /// An arrow's ends (cm): from its anchors, else its path.
    pub ends: Option<(Pt, Pt)>,
}

/// Snap a length to 0.1 cm.
pub fn snap_len(v: f32) -> f32 {
    ((v * 10.0).round() / 10.0).max(0.1)
}

/// Snap an angle to 15° (unless `free`), into (−180, 180].
pub fn snap_angle(a: f32, free: bool) -> f32 {
    let a = if free { a.round() } else { (a / 15.0).round() * 15.0 };
    let mut a = a.rem_euclid(360.0);
    if a > 180.0 {
        a -= 360.0;
    }
    if a.abs() < 1e-3 { 0.0 } else { a }
}

fn fmt_len(v: f32) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s.is_empty() || s == "-0" { "0".into() } else { s }
}

fn fmt_angle(a: f32) -> String {
    format!("{}°", a.round() as i32)
}

/// The changed item for a drag of `s.handle` to `p` (cm), and its readout.
/// `free`: ⌘ (no angle snapping); `centred`: ⌥ (a blob resizes about its centre).
pub fn drag(s: &DragStart, p: Pt, free: bool, centred: bool, cayley_radius: &dyn Fn(&Cayley, f32) -> Option<f32>) -> Option<(Item, String)> {
    let mut item = s.item.clone();
    match (s.handle.kind, &mut item) {
        (HKind::Rotate, it) => {
            let f = s.frame.as_ref()?;
            let a0 = angle_of(it);
            let turn = deg(sub(p, f.centre)) - deg(sub(s.grab, f.centre));
            let a = snap_angle(a0 + turn, free);
            set_angle(it, a);
            Some((item, fmt_angle(a)))
        }
        (HKind::Radius, Item::NGon(g)) => {
            let f = s.frame.as_ref()?;
            let r = snap_len(geom::dist(p, f.centre) / s.scale).max(0.3);
            g.radius = Some(r);
            Some((item, format!("{} cm", fmt_len(r))))
        }
        (HKind::Scale, Item::Cayley(c)) => {
            let f = s.frame.as_ref()?;
            let d = geom::dist(p, f.centre).max(0.05);
            let outer = d / s.scale;
            let r = cayley_radius(c, outer).map(snap_len).unwrap_or_else(|| snap_len(outer));
            c.radius = Some(r.max(0.3));
            Some((item, format!("radius {} cm", fmt_len(r.max(0.3)))))
        }
        (HKind::Corner(sx, sy), Item::Blob(b)) => {
            let f = s.frame.as_ref()?;
            let (a, bb) = (f.half.0.max(0.05), f.half.1.max(0.05));
            let l = rot(sub(p, f.centre), -f.angle);
            let free_sides = centred || !s.top_level;
            let new_half = |cur: f32, along: f32, sgn: i8| -> f32 {
                if sgn == 0 {
                    cur
                } else if free_sides {
                    along.abs()
                } else {
                    ((sgn as f32 * along + cur) / 2.0).max(0.1)
                }
            };
            let (a1, b1) = (new_half(a, l.x, sx), new_half(bb, l.y, sy));
            // the size= she will read, snapped; then the half extents it means
            let (w, h) = (snap_len(2.0 * a1 / s.scale).max(0.4), snap_len(2.0 * b1 / s.scale).max(0.4));
            let (a1, b1) = (w * s.scale / 2.0, h * s.scale / 2.0);
            b.size = Some((w, h));
            if !free_sides {
                // the opposite side stays where it was
                let shift = Pt { x: sx as f32 * (a1 - a), y: sy as f32 * (b1 - bb) };
                let c = add(f.centre, rot(shift, f.angle));
                b.pos = Some(geom::abs(Pt { x: c.x / s.scale, y: c.y / s.scale }));
            }
            Some((item, format!("{} × {} cm", fmt_len(w), fmt_len(h))))
        }
        (HKind::RotationSlide, Item::NGon(g)) => {
            let f = s.frame.as_ref()?;
            let a = deg(sub(p, f.centre));
            let e = layout::nearest_edge(g.n, a, f.angle);
            g.rotation_edge = if e == layout::rotation_default_edge(g.n) { None } else { Some(e) };
            Some((item, format!("edge {e}")))
        }
        (HKind::Bend, Item::Arrow(a)) => {
            let (from, to) = s.ends?;
            let d = sub(to, from);
            let len = d.x.hypot(d.y);
            if len < 1e-3 {
                return None;
            }
            let n = Pt { x: -d.y / len, y: d.x / len };
            // the middle's lift now, plus how far the pointer has gone sideways
            // (the handle needn't sit at the middle)
            let h0 = 0.2936 * len * a.bend.to_radians().sin();
            let h = h0 + (p.x - s.grab.x) * n.x + (p.y - s.grab.y) * n.y;
            // TikZ's bend θ (looseness 1) lifts the middle by 0.2936·d·sin θ
            let k = (h / (0.2936 * len)).clamp(-1.0, 1.0);
            let mut th = k.asin().to_degrees().clamp(-80.0, 80.0);
            if !free {
                for snap in [0.0f32, 30.0, -30.0, 45.0, -45.0] {
                    if (th - snap).abs() <= 6.0 {
                        th = snap;
                    }
                }
            }
            let th = th.round();
            a.bend = if th.abs() < 0.5 { 0.0 } else { th };
            let what = if a.bend == 0.0 { "straight".to_string() } else if a.bend > 0.0 { format!("bend left {}°", a.bend as i32) } else { format!("bend right {}°", -a.bend as i32) };
            Some((item, what))
        }
        (HKind::ArrowLabel, Item::Arrow(a)) => {
            let (from, to) = s.ends?;
            let d = sub(to, from);
            let len2 = d.x * d.x + d.y * d.y;
            if len2 < 1e-6 {
                return None;
            }
            let rel = sub(p, from);
            let t = (rel.x * d.x + rel.y * d.y) / len2;
            let side = d.x * rel.y - d.y * rel.x;
            // left of the direction of travel is tikz-cd's own side
            a.side = if side >= 0.0 { LabelSide::Auto } else { LabelSide::Swap };
            if a.bend < 0.0 {
                // (a right bend carries its label round the outside: keep her side)
            }
            a.label_pos = if t < 0.3 {
                LabelPos::NearStart
            } else if t > 0.7 {
                LabelPos::NearEnd
            } else {
                LabelPos::Middle
            };
            let side_s = if a.side == LabelSide::Swap { "swap" } else { "label left" };
            let pos_s = match a.label_pos {
                LabelPos::NearStart => " · near start",
                LabelPos::NearEnd => " · near end",
                LabelPos::Middle => "",
            };
            Some((item, format!("{side_s}{pos_s}")))
        }
        _ => None,
    }
}

fn deg(v: Pt) -> f32 {
    v.y.atan2(v.x).to_degrees()
}

/// An item's own `angle` (n-gon, blob, Cayley graph).
pub fn angle_of(it: &Item) -> f32 {
    match it {
        Item::NGon(g) => g.angle,
        Item::Blob(b) => b.angle,
        Item::Cayley(c) => c.angle,
        _ => 0.0,
    }
}

pub fn set_angle(it: &mut Item, a: f32) -> bool {
    match it {
        Item::NGon(g) => g.angle = a,
        Item::Blob(b) => b.angle = a,
        Item::Cayley(c) => c.angle = a,
        _ => return false,
    }
    true
}

/// Which label (index into `labels`) sits at vertex position `k` (1-based),
/// after `turn` and `flip` (FIG-MODEL's `layout::ngon_label_index`).
pub fn label_index_at(g: &NGon, k: u32) -> usize {
    layout::ngon_label_index(g, k)
}

/// Does a set `radius=` keep this Cayley graph's drawing (only its size
/// changes)? Not for the layouts layout picks only while the size is
/// automatic (A4's truncated tetrahedron, S4's rings fall back to a circle).
/// Compared on the vertex positions, each scaled to its farthest vertex
/// (a prism's inner ring moves 7 %, which is fine; A4's change is 190 %).
pub fn cayley_resizable(c: &Cayley) -> bool {
    use std::cell::RefCell;
    use std::collections::HashMap;
    thread_local! {
        static SEEN: RefCell<HashMap<String, bool>> = RefCell::new(HashMap::new());
    }
    let key = format!("{:?}|{:?}|{:?}", c.group, c.gens, c.layout);
    if let Some(v) = SEEN.with(|m| m.borrow().get(&key).copied()) {
        return v;
    }
    let ok = super::doc::core("cayley_resizable", || {
        let Ok(g) = ntex_figures::cayley::group(&c.group, &c.gens) else { return false };
        let norm = |r: Option<f32>| -> Vec<Pt> {
            let c2 = Cayley { radius: r, angle: 0.0, ..c.clone() };
            let pts = layout::cayley_geometry(&c2, &g).0;
            let far = pts.iter().map(|q| q.x.hypot(q.y)).fold(0.0f32, f32::max).max(1e-4);
            pts.iter().map(|q| Pt { x: q.x / far, y: q.y / far }).collect()
        };
        let (a, b) = (norm(None), norm(Some(1.0)));
        a.len() == b.len() && a.iter().zip(&b).all(|(p, q)| geom::dist(*p, *q) < 0.25)
    })
    .unwrap_or(false);
    SEEN.with(|m| m.borrow_mut().insert(key, ok));
    ok
}
