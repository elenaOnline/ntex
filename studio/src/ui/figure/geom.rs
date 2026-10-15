//! Geometry for the canvas: the cm ↔ px transform, the palette, snapping,
//! a local hit test (used when `recognize::hit` isn't built), and the
//! outlines of objects. No GPUI state; pure functions over the Scene.
use gpui::{Bounds, Hsla, Pixels, Point, point, px, rgb};
use ntex_figures::model::*;

/// pt per cm (TeX's 72.27 pt per inch).
pub const PT_PER_CM: f32 = 28.452_756;

/// Scene cm → window px. y up in cm, down in px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Xf {
    /// Where `center` lands, in window px.
    pub origin: Point<Pixels>,
    pub center: Pt,
    /// px per cm
    pub scale: f32,
}

impl Xf {
    pub fn to_px(&self, p: Pt) -> Point<Pixels> {
        point(self.origin.x + px((p.x - self.center.x) * self.scale), self.origin.y - px((p.y - self.center.y) * self.scale))
    }
    pub fn to_cm(&self, p: Point<Pixels>) -> Pt {
        Pt { x: self.center.x + f32::from(p.x - self.origin.x) / self.scale, y: self.center.y - f32::from(p.y - self.origin.y) / self.scale }
    }
    /// A length in pt, in px.
    pub fn pt(&self, pt: f32) -> Pixels {
        px(pt / PT_PER_CM * self.scale)
    }
    pub fn rect_px(&self, r: &Rect) -> Bounds<Pixels> {
        let a = self.to_px(Pt { x: r.min.x, y: r.max.y });
        let b = self.to_px(Pt { x: r.max.x, y: r.min.y });
        Bounds::from_corners(a, b)
    }
}

/// The zoom and pan the window keeps; the transform follows from the
/// canvas's bounds, so the draft and the render always agree.
#[derive(Clone, Debug, PartialEq)]
pub struct View {
    /// The cm rectangle that is fitted into the canvas at zoom 1.
    pub target: Rect,
    pub zoom: f32,
    /// px, added after fitting.
    pub pan: (f32, f32),
}

impl Default for View {
    fn default() -> View {
        View { target: empty_target(), zoom: 1.0, pan: (0.0, 0.0) }
    }
}

/// px per cm at zoom 1: never more than 2× the printed size (a figure in her
/// notes is read at about 1.3×), never so small that labels vanish.
pub const MAX_FIT: f32 = 48.0;
pub const MIN_FIT: f32 = 12.0;
/// Room around the fitted figure, px.
pub const FIT_PAD: f32 = 36.0;
/// Fitting (on open, on reload, ⌘0) zooms a small figure up to this, no further.
pub const FIT_ZOOM_MAX: f32 = 2.0;

fn empty_target() -> Rect {
    Rect { min: Pt { x: -3.0, y: -2.0 }, max: Pt { x: 3.0, y: 2.0 } }
}

impl View {
    /// px per cm that would make the target fill the canvas (unclamped).
    fn fill_scale(&self, bounds: Bounds<Pixels>) -> f32 {
        let w = (f32::from(bounds.size.width) - 2.0 * FIT_PAD).max(40.0);
        let h = (f32::from(bounds.size.height) - 2.0 * FIT_PAD).max(40.0);
        let tw = (self.target.max.x - self.target.min.x).max(0.5);
        let th = (self.target.max.y - self.target.min.y).max(0.5);
        (w / tw).min(h / th)
    }

    pub fn xf(&self, bounds: Bounds<Pixels>) -> Xf {
        let fit = self.fill_scale(bounds).clamp(MIN_FIT, MAX_FIT);
        let c = bounds.center();
        Xf {
            origin: point(c.x + px(self.pan.0), c.y + px(self.pan.1)),
            center: Pt { x: (self.target.min.x + self.target.max.x) / 2.0, y: (self.target.min.y + self.target.max.y) / 2.0 },
            scale: fit * self.zoom,
        }
    }

    /// Fit this scene (zoom 1, no pan).
    pub fn fit(&mut self, scene: &Scene) {
        self.target = if is_empty_rect(&scene.bounds) { empty_target() } else { scene.bounds.clone() };
        self.zoom = 1.0;
        self.pan = (0.0, 0.0);
    }

    /// Fit this scene to the canvas: centred, as large as fits, but never
    /// past FIT_ZOOM_MAX (a small figure isn't blown up to poster size). An
    /// empty scene gets the plain fit (zoom 1).
    pub fn fit_to(&mut self, scene: &Scene, bounds: Bounds<Pixels>) {
        self.fit(scene);
        if is_empty_rect(&scene.bounds) {
            return;
        }
        let fill = self.fill_scale(bounds);
        let at_one = fill.clamp(MIN_FIT, MAX_FIT);
        self.zoom = (fill / at_one).clamp(0.2, FIT_ZOOM_MAX);
    }

    /// Zoom by `factor` keeping `at` (window px) where it is.
    pub fn zoom_at(&mut self, factor: f32, at: Point<Pixels>, bounds: Bounds<Pixels>) {
        let before = self.xf(bounds);
        let p = before.to_cm(at);
        self.zoom = (self.zoom * factor).clamp(0.2, 8.0);
        let after = self.xf(bounds);
        let q = after.to_px(p);
        self.pan.0 += f32::from(at.x - q.x);
        self.pan.1 += f32::from(at.y - q.y);
    }

    /// The canvas changed size (the panel opened, a log line appeared): keep
    /// every point of the figure where it was on screen.
    pub fn keep(&mut self, from: Bounds<Pixels>, to: Bounds<Pixels>) {
        let old = self.xf(from);
        let mut one = self.clone();
        one.zoom = 1.0;
        one.pan = (0.0, 0.0);
        let s1 = one.xf(to).scale.max(1e-3);
        self.zoom = old.scale / s1;
        let c = to.center();
        self.pan = (f32::from(old.origin.x - c.x), f32::from(old.origin.y - c.y));
    }

    /// Does any of `r` (cm) show in the canvas?
    pub fn touches(&self, r: &Rect, bounds: Bounds<Pixels>) -> bool {
        if is_empty_rect(r) {
            return true;
        }
        let b = self.xf(bounds).rect_px(r);
        b.intersects(&bounds)
    }

    /// Is `r` (cm) inside the visible canvas?
    pub fn shows(&self, r: &Rect, bounds: Bounds<Pixels>) -> bool {
        if is_empty_rect(r) {
            return true;
        }
        let b = self.xf(bounds).rect_px(r);
        bounds.contains(&b.origin) && bounds.contains(&b.bottom_right())
    }
}

pub fn is_empty_rect(r: &Rect) -> bool {
    r.max.x <= r.min.x && r.max.y <= r.min.y
}

// ---------------------------------------------------------------------------
// the palette (ntex-base.sty / ntex-figure.sty)

/// A figure colour name → its Rule tone (strokes, dots, labels).
pub fn color(name: &str) -> Hsla {
    let c = match name.trim().to_ascii_lowercase().as_str() {
        "blue" => 0x1E47B8,
        "violet" => 0x661EB8,
        "teal" => 0x1C829C,
        "maroon" => 0xA92D4A,
        "green" => 0x238B45,
        "ochre" => 0xB07D12,
        "grey" | "gray" | "slate" => 0x5A6378,
        _ => 0x1E1E28,
    };
    rgb(c).into()
}

/// A colour name → its Back tint (a shaded blob).
pub fn tint(name: &str) -> Hsla {
    let n = name.trim().to_ascii_lowercase();
    // "blue-tint" (layout), "blue tint", "blue shade"
    let n = n.trim_end_matches("shade").trim_end_matches("tint").trim_end_matches(['-', ' ', '!']).trim();
    let c = match n {
        "blue" => 0xE0E7FB,
        "violet" => 0xECE0FB,
        "teal" => 0xE2F4F8,
        "maroon" => 0xF8EAED,
        "green" => 0xE5F5EA,
        "ochre" => 0xFAF3E2,
        _ => 0xF4F4F4,
    };
    rgb(c).into()
}

// ---------------------------------------------------------------------------
// positions and snapping

/// The grid cell in cm.
pub fn cell(scene: &Scene, fig: &Figure) -> (f32, f32) {
    if scene.cell.0 > 0.0 && scene.cell.1 > 0.0 {
        scene.cell
    } else {
        let s = fig.globals.scale.max(0.05);
        let (x, y) = fig.globals.spacing;
        if x > 0.0 && y > 0.0 { (x * s, y * s) } else { DEFAULT_SPACING }
    }
}

/// The nearest cell (or half cell).
pub fn snap(p: Pt, cell: (f32, f32), halves: bool) -> Pos {
    let q = if halves { 2.0 } else { 1.0 };
    let c = (p.x / cell.0 * q).round() / q;
    let r = (-p.y / cell.1 * q).round() / q;
    Pos::Cell(c + 0.0, r + 0.0)
}

pub fn pos_pt(pos: Pos, cell: (f32, f32)) -> Pt {
    match pos {
        Pos::Cell(c, r) => Pt { x: c * cell.0, y: -r * cell.1 },
        Pos::Abs(x, y) => Pt { x, y },
    }
}

/// Absolute centimetres, to a millimetre.
pub fn abs(p: Pt) -> Pos {
    let r = |v: f32| (v * 10.0).round() / 10.0 + 0.0;
    Pos::Abs(r(p.x), r(p.y))
}

// ---------------------------------------------------------------------------
// outlines

/// A path's points, with each Bézier flattened.
pub fn flatten(start: Pt, segs: &[Seg]) -> Vec<Pt> {
    let mut out = vec![start];
    let mut cur = start;
    for s in segs {
        match *s {
            Seg::Line(p) => {
                out.push(p);
                cur = p;
            }
            Seg::Cubic(a, b, e) => {
                for i in 1..=12 {
                    let t = i as f32 / 12.0;
                    let u = 1.0 - t;
                    out.push(Pt {
                        x: u * u * u * cur.x + 3.0 * u * u * t * a.x + 3.0 * u * t * t * b.x + t * t * t * e.x,
                        y: u * u * u * cur.y + 3.0 * u * u * t * a.y + 3.0 * u * t * t * b.y + t * t * t * e.y,
                    });
                }
                cur = e;
            }
        }
    }
    out
}

pub fn dist(a: Pt, b: Pt) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

fn seg_dist(p: Pt, a: Pt, b: Pt) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l2 = dx * dx + dy * dy;
    if l2 <= 1e-9 {
        return dist(p, a);
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / l2).clamp(0.0, 1.0);
    dist(p, Pt { x: a.x + t * dx, y: a.y + t * dy })
}

pub fn poly_dist(p: Pt, pts: &[Pt]) -> f32 {
    pts.windows(2).map(|w| seg_dist(p, w[0], w[1])).fold(f32::INFINITY, f32::min)
}

pub fn inside(p: Pt, poly: &[Pt]) -> bool {
    let mut c = false;
    let n = poly.len();
    if n < 3 {
        return false;
    }
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

fn in_rect(p: Pt, r: &Rect, slack: f32) -> bool {
    p.x >= r.min.x - slack && p.x <= r.max.x + slack && p.y >= r.min.y - slack && p.y <= r.max.y + slack
}

/// The topmost object under `p` (cm): labels and nodes, then paths, then
/// the insides of closed shapes. `tol` in cm.
pub fn hit_local(scene: &Scene, p: Pt, tol: f32) -> Option<ObjRef> {
    // anchors: the smallest box that contains the point
    let mut best: Option<(f32, &ObjRef)> = None;
    for (obj, _, r) in &scene.anchors {
        if in_rect(p, r, tol * 0.5) {
            let area = (r.max.x - r.min.x) * (r.max.y - r.min.y);
            if best.is_none_or(|(a, _)| area < a) {
                best = Some((area, obj));
            }
        }
    }
    if let Some((_, o)) = best {
        return Some(o.clone());
    }
    for d in scene.draws.iter().rev() {
        if let Draw::Label { obj, at, latex, size, .. } = d {
            let w = latex.chars().count().max(1) as f32 * size * 0.5 / PT_PER_CM;
            let h = size * 1.2 / PT_PER_CM;
            let r = Rect { min: Pt { x: at.x - w / 2.0, y: at.y - h / 2.0 }, max: Pt { x: at.x + w / 2.0, y: at.y + h / 2.0 } };
            if in_rect(p, &r, tol * 0.3) {
                return Some(obj.clone());
            }
        }
    }
    // strokes
    let mut near: Option<(f32, &ObjRef)> = None;
    for d in &scene.draws {
        if let Draw::Path { obj, start, segs, stroke: Some(_), .. } = d {
            let pts = flatten(*start, segs);
            let dd = poly_dist(p, &pts);
            if dd <= tol && near.is_none_or(|(b, _)| dd < b) {
                near = Some((dd, obj));
            }
        }
    }
    if let Some((_, o)) = near {
        return Some(o.clone());
    }
    // insides: the innermost closed shape
    let mut inner: Option<(f32, &ObjRef)> = None;
    for d in &scene.draws {
        if let Draw::Path { obj, start, segs, closed: true, .. } = d {
            let pts = flatten(*start, segs);
            if inside(p, &pts) {
                let r = bbox(&pts);
                let area = (r.max.x - r.min.x) * (r.max.y - r.min.y);
                if inner.is_none_or(|(a, _)| area < a) {
                    inner = Some((area, obj));
                }
            }
        }
    }
    inner.map(|(_, o)| o.clone())
}

pub fn bbox(pts: &[Pt]) -> Rect {
    let mut r = Rect { min: Pt { x: f32::INFINITY, y: f32::INFINITY }, max: Pt { x: f32::NEG_INFINITY, y: f32::NEG_INFINITY } };
    for p in pts {
        r.min.x = r.min.x.min(p.x);
        r.min.y = r.min.y.min(p.y);
        r.max.x = r.max.x.max(p.x);
        r.max.y = r.max.y.max(p.y);
    }
    if pts.is_empty() { Rect::default() } else { r }
}

/// Where an object is: its anchor, or the middle of what it draws.
pub fn center_of(scene: &Scene, obj: &ObjRef) -> Option<Pt> {
    if let Some((_, p, _)) = scene.anchors.iter().find(|(o, _, _)| o == obj) {
        return Some(*p);
    }
    let r = extent_of(scene, obj)?;
    Some(Pt { x: (r.min.x + r.max.x) / 2.0, y: (r.min.y + r.max.y) / 2.0 })
}

/// The box around everything an object draws.
pub fn extent_of(scene: &Scene, obj: &ObjRef) -> Option<Rect> {
    if let Some((_, _, r)) = scene.anchors.iter().find(|(o, _, _)| o == obj) {
        return Some(r.clone());
    }
    let mut pts = Vec::new();
    for d in &scene.draws {
        match d {
            Draw::Path { obj: o, start, segs, .. } if o == obj => pts.extend(flatten(*start, segs)),
            Draw::Dot { obj: o, at, .. } if o == obj => pts.push(*at),
            _ => {}
        }
    }
    (!pts.is_empty()).then(|| bbox(&pts))
}

/// Where an arrow's label goes: the middle of its path.
pub fn path_mid(scene: &Scene, obj: &ObjRef) -> Option<Pt> {
    for d in &scene.draws {
        if let Draw::Path { obj: o, start, segs, .. } = d
            && o == obj
        {
            let pts = flatten(*start, segs);
            let total: f32 = pts.windows(2).map(|w| dist(w[0], w[1])).sum();
            let mut acc = 0.0;
            for w in pts.windows(2) {
                let l = dist(w[0], w[1]);
                if acc + l >= total / 2.0 && l > 0.0 {
                    let t = (total / 2.0 - acc) / l;
                    return Some(Pt { x: w[0].x + t * (w[1].x - w[0].x), y: w[0].y + t * (w[1].y - w[0].y) });
                }
                acc += l;
            }
            return pts.first().copied();
        }
    }
    None
}

/// The unit direction from a path's start to its end (cm).
pub fn path_dir(scene: &Scene, obj: &ObjRef) -> Option<(f32, f32)> {
    scene.draws.iter().find_map(|d| match d {
        Draw::Path { obj: o, start, segs, .. } if o == obj => {
            let pts = flatten(*start, segs);
            let (a, b) = (pts.first()?, pts.last()?);
            let l = dist(*a, *b);
            (l > 1e-4).then(|| ((b.x - a.x) / l, (b.y - a.y) / l))
        }
        _ => None,
    })
}

/// The name an arrow statement uses for an object, if it can be an end.
pub fn end_name(obj: &ObjRef) -> Option<String> {
    match obj {
        ObjRef::Node(n) | ObjRef::Blob(n) | ObjRef::Element(n) => Some(n.clone()),
        ObjRef::NGonVertex(p, k) => Some(format!("{p}.{k}")),
        ObjRef::CayleyVertex(c, w) => Some(format!("{c}.{w}")),
        _ => None,
    }
}

/// The outline of a recognised gesture, for the ghost (cm).
pub fn ghost_outline(r: &Recognized, scene: &Scene) -> Vec<Vec<Pt>> {
    use std::f32::consts::PI;
    match r {
        Recognized::Blob { center, size, .. } => {
            let (a, b) = (size.0 / 2.0, size.1 / 2.0);
            vec![(0..=48).map(|i| {
                let t = i as f32 / 48.0 * 2.0 * PI;
                Pt { x: center.x + a * t.cos(), y: center.y + b * t.sin() }
            })
            .collect()]
        }
        Recognized::NGon { n, center, radius } => {
            let n = (*n).max(3);
            // the vertices where layout puts them (even n on a flat bottom edge)
            vec![(0..=n).map(|i| {
                let t = ntex_figures::layout::ngon_vertex_angle(n, i % n + 1).to_radians();
                Pt { x: center.x + radius * t.cos(), y: center.y + radius * t.sin() }
            })
            .collect()]
        }
        Recognized::Arrow { from, to } => {
            let find = |name: &str| {
                scene.anchors.iter().find(|(o, _, _)| end_name(o).as_deref() == Some(name)).map(|(_, p, _)| *p)
            };
            match (find(from), find(to)) {
                (Some(a), Some(b)) => vec![vec![a, b]],
                _ => vec![],
            }
        }
        Recognized::Node(pos) => {
            let c = pos_pt(*pos, if scene.cell.0 > 0.0 { scene.cell } else { DEFAULT_SPACING });
            vec![(0..=24).map(|i| {
                let t = i as f32 / 24.0 * 2.0 * PI;
                Pt { x: c.x + 0.25 * t.cos(), y: c.y + 0.25 * t.sin() }
            })
            .collect()]
        }
        Recognized::Nothing => vec![],
    }
}
