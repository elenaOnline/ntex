//! The canvas: the exact render when idle, a native draft of the Scene
//! while you drag or type, and the tools' feedback on top.
//!
//! The draft draws Scene paths as GPUI paths and labels as
//! `unicode::preview_runs` text in XCharter, through the same cm → px transform
//! ([`geom::View::xf`]) the render is placed with, so swapping one for the
//! other doesn't move anything.
use super::geom::{self, PT_PER_CM, View, Xf};
use super::handles::{HKind, Handle};
use gpui::{CursorStyle, 
    App, Bounds, ContentMask, DispatchPhase, Font, FontFallbacks, FontStyle, FontWeight, Hitbox, HitboxBehavior, Hsla,
    IntoElement, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PathBuilder, PinchEvent, Pixels, Point, RenderImage,
    ScrollWheelEvent, Styled as _, TextAlign, TextRun, Window, canvas, fill, point, px, quad, size,
};
use ntex_figures::model::*;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

/// What the tools draw over the figure.
#[derive(Clone, Debug, Default)]
pub enum Overlay {
    #[default]
    None,
    /// The arrow tool's rubber band.
    Line(Pt, Pt),
    /// The blob tool's rectangle.
    Rect(Pt, Pt),
    /// The rough tool's stroke so far.
    Stroke(Vec<Pt>),
    /// The select tool's box.
    Marquee(Pt, Pt),
    /// (figure-qol D) A dragged arrow end's tether: dashed, no head.
    Tether(Pt, Pt),
}

/// The exact render: the image, its size in pt, and the scene bounds it
/// was made for (it is centred on them).
#[derive(Clone)]
pub struct Exact {
    pub image: Arc<RenderImage>,
    pub w_pt: f32,
    pub h_pt: f32,
    pub bounds: Rect,
}

/// Everything one canvas frame needs (a snapshot; the view keeps the state).
#[derive(Clone, Default)]
pub struct Frame {
    pub scene: Arc<Scene>,
    pub view: View,
    pub exact: Option<Exact>,
    /// Cell grid dots (select and node tools).
    pub grid: Option<(f32, f32)>,
    pub selection: Vec<ObjRef>,
    /// Highlighted (the arrow tool's target, the hovered object).
    pub hover: Option<ObjRef>,
    pub overlay: Overlay,
    /// The recognised shape's outline and its opacity (the 150 ms flash).
    pub ghost: Option<(Vec<Vec<Pt>>, f32)>,
    /// A label being edited in place: not drawn under the field.
    pub editing: Option<ObjRef>,
    pub empty_hint: Option<String>,
    /// A fit is due (the figure just opened or reloaded): paint it fitted to
    /// these bounds already; the window adopts the same fit on its next pump.
    pub fit: bool,
    /// The selection's handles (cm) at a given px per cm, so the fit keeps them in view.
    pub fit_handles: Option<Arc<dyn Fn(f32) -> Vec<Pt>>>,
    /// The bounds the view was set for: if the canvas is now another size,
    /// paint the figure where it was on screen (the view adopts it next).
    pub prev_bounds: Option<Bounds<Pixels>>,
    /// The selected n-gon's circumcircle (centre, radius cm): a faint
    /// dashed ring, so its resize dot reads as "on the circle".
    pub ring: Option<(Pt, f32)>,
    /// The selected object's handles (cm).
    pub handles: Vec<Handle>,
    /// A handle drag's live readout: (where the pointer is, "45°").
    pub readout: Option<(Pt, String)>,
    /// The cursor over the canvas (a handle's, a drag's).
    pub cursor: Option<CursorStyle>,
    /// (figure-qol D) `hover` is a drop target for a dragged arrow end: outlined firmly.
    pub drop_target: bool,
    /// (figure-qol B) The page frame (cm): shown while ⌘ is held; its corners scale the figure.
    pub page: Option<Rect>,
}

/// The mouse, routed to the view (window px; the view converts with `xf`).
pub struct Handlers {
    pub down: Box<dyn Fn(&MouseDownEvent, Xf, &mut Window, &mut App)>,
    pub moved: Box<dyn Fn(&MouseMoveEvent, Xf, &mut Window, &mut App)>,
    pub up: Box<dyn Fn(&MouseUpEvent, Xf, &mut Window, &mut App)>,
    pub scroll: Box<dyn Fn(&ScrollWheelEvent, Bounds<Pixels>, &mut Window, &mut App)>,
    pub pinch: Box<dyn Fn(&PinchEvent, Bounds<Pixels>, &mut Window, &mut App)>,
}

/// The canvas element. `bounds_out` gets the canvas bounds every frame
/// (the view converts key-driven zooms and places the label field with it).
pub fn element(frame: Frame, handlers: Handlers, bounds_out: Rc<Cell<Option<Bounds<Pixels>>>>) -> impl IntoElement {
    let handlers = Rc::new(handlers);
    canvas(
        move |bounds, window, _cx| window.insert_hitbox(bounds, HitboxBehavior::Normal),
        move |bounds, hitbox: Hitbox, window, cx| {
            bounds_out.set(Some(bounds));
            let xf = if frame.fit {
                let mut view = frame.view.clone();
                match &frame.fit_handles {
                    Some(h) => view.fit_to_with(&frame.scene, bounds, &**h),
                    None => view.fit_to(&frame.scene, bounds),
                }
                view.xf(bounds)
            } else if let Some(prev) = frame.prev_bounds
                && prev != bounds
            {
                let mut view = frame.view.clone();
                view.keep(prev, bounds);
                view.xf(bounds)
            } else {
                frame.view.xf(bounds)
            };
            if let Some(c) = frame.cursor {
                window.set_cursor_style(c, &hitbox);
            }
            window.with_content_mask(Some(ContentMask { bounds }), |window| paint(&frame, xf, bounds, window, cx));
            let h = handlers.clone();
            window.on_mouse_event(move |ev: &MouseDownEvent, phase, window, cx| {
                if phase == DispatchPhase::Bubble && hitbox.is_hovered(window) {
                    (h.down)(ev, xf, window, cx);
                }
            });
            let h = handlers.clone();
            window.on_mouse_event(move |ev: &MouseMoveEvent, phase, window, cx| {
                if phase == DispatchPhase::Bubble {
                    (h.moved)(ev, xf, window, cx);
                }
            });
            let h = handlers.clone();
            window.on_mouse_event(move |ev: &MouseUpEvent, phase, window, cx| {
                if phase == DispatchPhase::Bubble {
                    (h.up)(ev, xf, window, cx);
                }
            });
            let h = handlers.clone();
            window.on_mouse_event(move |ev: &ScrollWheelEvent, phase, window, cx| {
                if phase == DispatchPhase::Bubble && bounds.contains(&ev.position) {
                    (h.scroll)(ev, bounds, window, cx);
                }
            });
            let h = handlers.clone();
            window.on_mouse_event(move |ev: &PinchEvent, phase, window, cx| {
                if phase == DispatchPhase::Bubble && bounds.contains(&ev.position) {
                    (h.pinch)(ev, bounds, window, cx);
                }
            });
        },
    )
    .size_full()
}

fn blue() -> Hsla {
    gpui::rgb(0x1E47B8).into()
}

fn slate() -> Hsla {
    gpui::rgb(0x5A6378).into()
}

fn with_alpha(c: Hsla, a: f32) -> Hsla {
    Hsla { a: c.a * a, ..c }
}

/// One canvas frame.
pub fn paint(frame: &Frame, xf: Xf, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    window.paint_quad(fill(bounds, gpui::rgb(0xFFFFFF)));
    if let Some(cell) = frame.grid {
        paint_grid(cell, &frame.scene, xf, bounds, window);
    }
    match &frame.exact {
        Some(ex) => {
            let c = Pt { x: (ex.bounds.min.x + ex.bounds.max.x) / 2.0, y: (ex.bounds.min.y + ex.bounds.max.y) / 2.0 };
            let (w, h) = (ex.w_pt / PT_PER_CM * xf.scale, ex.h_pt / PT_PER_CM * xf.scale);
            let at = xf.to_px(c);
            let img = Bounds::new(point(at.x - px(w / 2.0), at.y - px(h / 2.0)), size(px(w), px(h)));
            // white is knocked out, so the dots stay under the ink
            let _ = window.paint_image(bounds, img, Default::default(), ex.image.clone(), 0, false);
        }
        None => paint_scene(&frame.scene, &xf, &Look { alpha: 1.0, color: None, skip_label: frame.editing.clone() }, window, cx),
    }
    // hover: a faint outline on what a click would take
    if let Some(obj) = &frame.hover
        && !frame.selection.contains(obj)
    {
        if matches!(obj, ObjRef::Arrow(_) | ObjRef::CayleyEdge(..) | ObjRef::NGonRotation(_)) {
            for d in &frame.scene.draws {
                if let Draw::Path { obj: o, stroke: Some(_), .. } = d
                    && o == obj
                {
                    paint_draw(d, &xf, &Look { alpha: 0.45, color: Some(blue()), skip_label: None }, window, cx);
                }
            }
        } else if let Some(t) = super::handles::turned_outline(&frame.scene, obj) {
            let (w, a) = if frame.drop_target { (2.0, 0.95) } else { (1.0, 0.38) };
            paint_turned(&t, &xf, w, with_alpha(blue(), a), window);
        } else if let Some(r) = super::handles::outline(&frame.scene, obj) {
            let b = xf.rect_px(&r).dilate(px(4.));
            let (w, a) = if frame.drop_target { (2.0, 0.95) } else { (1.0, 0.38) };
            window.paint_quad(quad(b, px(0.), with_alpha(blue(), if frame.drop_target { 0.08 } else { 0.0 }), px(w), with_alpha(blue(), a), Default::default()));
        }
    }
    // selection: a 1.5 px blue outline around each selected object; arrows redrawn in blue
    for obj in &frame.selection {
        paint_selected(&frame.scene, obj, &xf, window, cx);
    }
    if let Some((c, r)) = frame.ring {
        let pts: Vec<_> = (0..=96).map(|i| {
            let t = i as f32 / 96.0 * std::f32::consts::TAU;
            xf.to_px(Pt { x: c.x + r * t.cos(), y: c.y + r * t.sin() })
        }).collect();
        stroke_poly(&pts, px(1.), with_alpha(blue(), 0.45), Some(&[px(3.), px(3.)]), window);
    }
    paint_handles(&frame.handles, &frame.scene, &xf, window, cx);
    if let Some(page) = &frame.page {
        // the sheet the figure is cut from: a faint rule and a grip at each corner
        let r = xf.rect_px(page);
        window.paint_quad(quad(r, px(0.), with_alpha(slate(), 0.03), px(1.), with_alpha(slate(), 0.55), Default::default()));
        for (x, y) in [(r.left(), r.top()), (r.right(), r.top()), (r.left(), r.bottom()), (r.right(), r.bottom())] {
            let g = Bounds::new(point(x - px(4.5), y - px(4.5)), size(px(9.), px(9.)));
            window.paint_quad(quad(g, px(1.5), gpui::rgb(0xFFFFFF), px(1.2), blue(), Default::default()));
        }
    }
    match &frame.overlay {
        Overlay::None => {}
        Overlay::Line(a, b) => {
            // from the edge of the box it starts in, not through its label
            let from = frame
                .scene
                .anchors
                .iter()
                .find(|(_, p, _)| geom::dist(*p, *a) < 1e-4)
                .map(|(_, _, r)| clip_to_box(*a, *b, r))
                .unwrap_or(*a);
            stroke_poly(&[xf.to_px(from), xf.to_px(*b)], px(1.2), blue(), Some(&[px(4.), px(3.)]), window);
            paint_tip(Head::To, xf.to_px(*b), dir_px(xf.to_px(from), xf.to_px(*b)), px(1.2), tip_len(&xf), blue(), window);
        }
        Overlay::Tether(a, b) => {
            stroke_poly(&[xf.to_px(*a), xf.to_px(*b)], px(1.2), blue(), Some(&[px(4.), px(3.)]), window);
        }
        Overlay::Rect(a, b) => {
            let r = Bounds::from_corners(xf.to_px(Pt { x: a.x.min(b.x), y: a.y.max(b.y) }), xf.to_px(Pt { x: a.x.max(b.x), y: a.y.min(b.y) }));
            window.paint_quad(quad(r, px(0.), with_alpha(blue(), 0.05), px(1.), with_alpha(blue(), 0.8), gpui::BorderStyle::Dashed));
        }
        Overlay::Stroke(pts) => {
            let p: Vec<_> = pts.iter().map(|p| xf.to_px(*p)).collect();
            stroke_poly(&p, px(1.5), with_alpha(slate(), 0.8), None, window);
        }
        Overlay::Marquee(a, b) => {
            let r = Bounds::from_corners(xf.to_px(Pt { x: a.x.min(b.x), y: a.y.max(b.y) }), xf.to_px(Pt { x: a.x.max(b.x), y: a.y.min(b.y) }));
            window.paint_quad(quad(r, px(0.), with_alpha(blue(), 0.04), px(1.), with_alpha(blue(), 0.7), gpui::BorderStyle::Dashed));
        }
    }
    if let Some((at, text)) = &frame.readout {
        // (review U15b) the readout keeps clear of the figure's labels
        let labels: Vec<Bounds<Pixels>> = frame
            .scene
            .draws
            .iter()
            .filter_map(|d| match d {
                Draw::Label { at, latex, size, math, .. } => Some(xf.rect_px(&super::handles::label_rect(*at, latex, *size, *math))),
                _ => None,
            })
            .collect();
        paint_readout(text, xf.to_px(*at), bounds, &labels, window, cx);
    }
    if let Some((outlines, a)) = &frame.ghost {
        for o in outlines {
            let p: Vec<_> = o.iter().map(|p| xf.to_px(*p)).collect();
            stroke_poly(&p, px(2.), with_alpha(blue(), *a), None, window);
        }
    }
    if let Some(hint) = &frame.empty_hint
        && frame.scene.draws.is_empty()
    {
        let t = cx.global::<crate::ui::widgets::theme::Theme>().clone();
        let run = TextRun { len: hint.len(), font: t.serif_italic(), color: t.slate, background_color: None, underline: None, strikethrough: None };
        let line = window.text_system().shape_line(hint.clone().into(), t.body_size, &[run], None);
        let c = bounds.center();
        let _ = line.paint(point(c.x - line.width() / 2.0, c.y - t.line(t.body_size) / 2.0), t.line(t.body_size), TextAlign::Left, None, window, cx);
    }
}

/// Where the segment from `a` (inside `r`) towards `b` leaves the box.
fn clip_to_box(a: Pt, b: Pt, r: &Rect) -> Pt {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let mut t: f32 = 1.0;
    if dx > 1e-6 {
        t = t.min((r.max.x - a.x) / dx);
    } else if dx < -1e-6 {
        t = t.min((r.min.x - a.x) / dx);
    }
    if dy > 1e-6 {
        t = t.min((r.max.y - a.y) / dy);
    } else if dy < -1e-6 {
        t = t.min((r.min.y - a.y) / dy);
    }
    let t = t.clamp(0.0, 1.0);
    Pt { x: a.x + dx * t, y: a.y + dy * t }
}

fn paint_grid(cell: (f32, f32), scene: &Scene, xf: Xf, bounds: Bounds<Pixels>, window: &mut Window) {
    let a = xf.to_cm(bounds.origin);
    let b = xf.to_cm(bounds.bottom_right());
    let (c0, c1) = ((a.x / cell.0).floor() as i32, (b.x / cell.0).ceil() as i32);
    let (r0, r1) = ((-a.y / cell.1).floor() as i32, (-b.y / cell.1).ceil() as i32);
    if (c1 - c0) * (r1 - r0) > 4000 {
        return;
    }
    let dot = with_alpha(slate(), 0.45);
    for c in c0..=c1 {
        for r in r0..=r1 {
            let q = Pt { x: c as f32 * cell.0, y: -(r as f32) * cell.1 };
            // never inside a node's box (a dot in the middle of a letter)
            if scene.anchors.iter().any(|(o, _, b)| {
                matches!(o, ObjRef::Node(_) | ObjRef::Element(_) | ObjRef::CayleyVertex(..) | ObjRef::NGonVertex(..))
                    && q.x >= b.min.x - 0.05
                    && q.x <= b.max.x + 0.05
                    && q.y >= b.min.y - 0.05
                    && q.y <= b.max.y + 0.05
            }) {
                continue;
            }
            let p = xf.to_px(q);
            window.paint_quad(quad(Bounds::new(point(p.x - px(1.25), p.y - px(1.25)), size(px(2.5), px(2.5))), px(1.25), dot, px(0.), dot, Default::default()));
        }
    }
}

/// How a scene is drawn: the draft (alpha 1), a ghost or the selection
/// (one colour).
pub struct Look {
    pub alpha: f32,
    pub color: Option<Hsla>,
    pub skip_label: Option<ObjRef>,
}

pub fn paint_scene(scene: &Scene, xf: &Xf, look: &Look, window: &mut Window, cx: &mut App) {
    for d in &scene.draws {
        paint_draw(d, xf, look, window, cx);
    }
}

fn pick(name: &str, look: &Look) -> Hsla {
    with_alpha(look.color.unwrap_or_else(|| geom::color(name)), look.alpha)
}

pub fn paint_draw(d: &Draw, xf: &Xf, look: &Look, window: &mut Window, cx: &mut App) {
    match d {
        Draw::Path { start, segs, closed, stroke, fill: fill_name, fill_opacity, start_tip, end_tip, .. } => {
            let pts_px = |b: &mut PathBuilder| {
                b.move_to(xf.to_px(*start));
                for s in segs {
                    match *s {
                        Seg::Line(p) => b.line_to(xf.to_px(p)),
                        Seg::Cubic(c1, c2, e) => b.cubic_bezier_to(xf.to_px(e), xf.to_px(c1), xf.to_px(c2)),
                    }
                }
                if *closed {
                    b.close();
                }
            };
            if let Some(f) = fill_name
                && look.color.is_none()
            {
                let mut b = PathBuilder::fill();
                pts_px(&mut b);
                if let Ok(p) = b.build() {
                    let op = if *fill_opacity > 0.0 { *fill_opacity } else { 1.0 };
                    // `<name>-tint` is the clear Back tone; a bare name the vivid one
                    let c = if f.ends_with("tint") { geom::tint(f) } else { geom::color(f) };
                    window.paint_path(p, with_alpha(c, op * look.alpha));
                }
            }
            let Some(st) = stroke else { return };
            let colour = pick(&st.color, look);
            let w = xf.pt(st.width).max(px(0.9));
            let dash: Option<Vec<Pixels>> = match st.line {
                Line::Solid => None,
                Line::Dashed => Some(vec![xf.pt(3.0).max(px(3.)), xf.pt(3.0).max(px(3.))]),
                Line::Dotted => Some(vec![w, xf.pt(2.0).max(px(2.))]),
            };
            if st.double {
                // tikz-cd's double line: two rules with a gap
                let gap = xf.pt(1.6).max(px(2.));
                let mut b = PathBuilder::stroke(gap + w * 2.0);
                if let Some(d) = &dash {
                    b = b.dash_array(d);
                }
                pts_px(&mut b);
                if let Ok(p) = b.build() {
                    window.paint_path(p, colour);
                }
                let mut b = PathBuilder::stroke(gap);
                pts_px(&mut b);
                if let Ok(p) = b.build() {
                    window.paint_path(p, gpui::rgb(0xFFFFFF));
                }
            } else {
                let mut b = PathBuilder::stroke(w);
                if let Some(d) = &dash {
                    b = b.dash_array(d);
                }
                pts_px(&mut b);
                if let Ok(p) = b.build() {
                    window.paint_path(p, colour);
                }
            }
            // tips: the direction at each end
            let pts = geom::flatten(*start, segs);
            if pts.len() >= 2 {
                let n = pts.len();
                let end = xf.to_px(pts[n - 1]);
                let before_end = back_from(&pts, xf, true);
                let s = xf.to_px(pts[0]);
                let after_start = back_from(&pts, xf, false);
                let fwd_end = dir_px(before_end, end);
                let fwd_start = dir_px(s, after_start); // direction of travel at the start
                let big = if st.double { 1.35 } else { 1.0 };
                let len = tip_len(xf) * big;
                let mut start_deco = start_tip.filter(|h| matches!(h, Head::Tail | Head::Hook | Head::MapsTo));
                if let Some(h) = end_tip {
                    match h {
                        Head::To | Head::Both | Head::Double => paint_tip(Head::To, end, fwd_end, w, len, colour, window),
                        Head::TwoHeads => paint_tip(Head::TwoHeads, end, fwd_end, w, len, colour, window),
                        Head::Tail | Head::Hook | Head::MapsTo => {
                            paint_tip(Head::To, end, fwd_end, w, len, colour, window);
                            start_deco = start_deco.or(Some(*h));
                        }
                        Head::Equal => {}
                    }
                }
                if let Some(h) = start_tip
                    && matches!(h, Head::To | Head::Both | Head::Double | Head::TwoHeads)
                {
                    let back = (-fwd_start.0, -fwd_start.1);
                    paint_tip(*h, s, back, w, len, colour, window);
                }
                if let Some(h) = start_deco {
                    paint_tip(h, s, fwd_start, w, len, colour, window);
                }
            }
        }
        Draw::Dot { at, r, color, .. } => {
            let c = xf.to_px(*at);
            // r in cm (the Scene's unit; a value over 0.5 can only be pt);
            // never smaller than a visible dot
            let rr = if *r > 0.5 { xf.pt(*r) } else { px(*r * xf.scale) }.max(px(1.6));
            let colour = pick(color, look);
            window.paint_quad(quad(Bounds::new(point(c.x - rr, c.y - rr), size(rr * 2.0, rr * 2.0)), rr, colour, px(0.), colour, Default::default()));
        }
        Draw::Label { obj, at, latex, color, size: pt, math } => {
            if look.skip_label.as_ref() == Some(obj) {
                return;
            }
            let styled = super::doc::preview_runs(latex, *math);
            if styled.is_empty() {
                return;
            }
            let colour = pick(color, look);
            let size_pt = if *pt > 0.0 { *pt } else { 10.0 };
            let fs = xf.pt(size_pt);
            // where TikZ puts the baseline: a diagram node's `at` is the maths
            // axis (2.5 pt up at 10 pt); every other label is centred on its box
            let k = size_pt / 10.0;
            let up_pt = if matches!(obj, ObjRef::Node(_)) {
                2.5 * k
            } else {
                let (_, ht, dp) = ntex_figures::layout::label_metrics(latex, *math);
                (ht - dp) / 2.0 * k
            };
            let at_px = xf.to_px(*at);
            paint_label(&styled, point(at_px.x, at_px.y + xf.pt(up_pt)), fs, colour, window, cx);
        }
    }
}

/// A point a little way back along the path from an end (for the tip's
/// direction), so a curved arrow's tip follows the curve.
fn back_from(pts: &[Pt], xf: &Xf, from_end: bool) -> Point<Pixels> {
    let n = pts.len();
    let anchor = if from_end { pts[n - 1] } else { pts[0] };
    let want = 4.0 / xf.scale.max(1.0); // 4 px
    let iter: Box<dyn Iterator<Item = &Pt>> = if from_end { Box::new(pts.iter().rev().skip(1)) } else { Box::new(pts.iter().skip(1)) };
    let mut last = anchor;
    for p in iter {
        last = *p;
        if geom::dist(*p, anchor) >= want {
            break;
        }
    }
    xf.to_px(last)
}

fn dir_px(a: Point<Pixels>, b: Point<Pixels>) -> (f32, f32) {
    let (dx, dy) = (f32::from(b.x - a.x), f32::from(b.y - a.y));
    let l = (dx * dx + dy * dy).sqrt();
    if l < 1e-4 { (1.0, 0.0) } else { (dx / l, dy / l) }
}

fn tip_len(xf: &Xf) -> Pixels {
    xf.pt(4.2).max(px(5.))
}

/// tikz-cd's tips: `cm to` barbs, two heads, the tail, the hook, the bar.
/// `d` is the direction of travel at `at`.
fn paint_tip(head: Head, at: Point<Pixels>, d: (f32, f32), w: Pixels, len: Pixels, colour: Hsla, window: &mut Window) {
    let l = f32::from(len);
    let n = (-d.1, d.0);
    let p = |along: f32, side: f32| point(at.x + px(d.0 * along + n.0 * side), at.y + px(d.1 * along + n.1 * side));
    let barbs = |tip_back: f32, window: &mut Window| {
        // cm to: two barbs curving back from the tip
        let mut b = PathBuilder::stroke(w);
        b.move_to(p(-tip_back - l, l * 0.72));
        b.curve_to(p(-tip_back, 0.0), p(-tip_back - l * 0.35, l * 0.1));
        b.curve_to(p(-tip_back - l, -l * 0.72), p(-tip_back - l * 0.35, -l * 0.1));
        if let Ok(path) = b.build() {
            window.paint_path(path, colour);
        }
    };
    match head {
        Head::To | Head::Both | Head::Double => barbs(0.0, window),
        Head::TwoHeads => {
            barbs(0.0, window);
            barbs(l * 0.75, window);
        }
        // >->: the same barbs at the start, pointing along the arrow
        Head::Tail => barbs(-l * 0.95, window),
        Head::Hook => {
            let r = l * 0.5;
            let mut b = PathBuilder::stroke(w);
            b.move_to(p(0.0, 0.0));
            b.arc_to(point(px(r), px(r)), px(0.), false, true, p(0.0, 2.0 * r));
            if let Ok(path) = b.build() {
                window.paint_path(path, colour);
            }
        }
        Head::MapsTo => {
            let mut b = PathBuilder::stroke(w);
            b.move_to(p(0.0, l * 0.62));
            b.line_to(p(0.0, -l * 0.62));
            if let Ok(path) = b.build() {
                window.paint_path(path, colour);
            }
        }
        Head::Equal => {}
    }
}

fn stroke_poly(pts: &[Point<Pixels>], w: Pixels, colour: Hsla, dash: Option<&[Pixels]>, window: &mut Window) {
    if pts.len() < 2 {
        return;
    }
    let mut b = PathBuilder::stroke(w);
    if let Some(d) = dash {
        b = b.dash_array(d);
    }
    b.move_to(pts[0]);
    for p in &pts[1..] {
        b.line_to(*p);
    }
    if let Ok(p) = b.build() {
        window.paint_path(p, colour);
    }
}

fn paint_selected(scene: &Scene, obj: &ObjRef, xf: &Xf, window: &mut Window, cx: &mut App) {
    let blue = blue();
    let mut drew_path = false;
    for d in &scene.draws {
        if let Draw::Path { obj: o, stroke: Some(_), .. } = d
            && o == obj
            && matches!(obj, ObjRef::Arrow(_) | ObjRef::CayleyEdge(..) | ObjRef::NGonRotation(_))
        {
            paint_draw(d, xf, &Look { alpha: 1.0, color: Some(blue), skip_label: None }, window, cx);
            drew_path = true;
        }
    }
    if drew_path {
        return;
    }
    if let Some(t) = super::handles::turned_outline(scene, obj) {
        paint_turned(&t, xf, 1.5, blue, window);
    } else if let Some(r) = super::handles::outline(scene, obj) {
        let b = xf.rect_px(&r).dilate(px(4.));
        window.paint_quad(quad(b, px(0.), with_alpha(blue, 0.0), px(1.5), blue, Default::default()));
    }
}

/// (figure-qol A) A turned shape's box, turned with it, 4 px clear of what it draws.
fn paint_turned(t: &super::handles::Turned, xf: &Xf, w: f32, colour: Hsla, window: &mut Window) {
    let c = t.corners(4.0 / xf.scale.max(1.0));
    let mut pts: Vec<Point<Pixels>> = c.iter().map(|q| xf.to_px(*q)).collect();
    pts.push(pts[0]);
    stroke_poly(&pts, px(w), colour, None, window);
}

/// The cursor a handle shows.
pub fn cursor_for(k: HKind, dragging: bool) -> CursorStyle {
    match k {
        HKind::Rotate => {
            if dragging {
                CursorStyle::ClosedHand
            } else {
                CursorStyle::OpenHand
            }
        }
        HKind::Corner(0, _) => CursorStyle::ResizeUpDown,
        HKind::Corner(_, 0) => CursorStyle::ResizeLeftRight,
        HKind::Corner(sx, sy) if sx == sy => CursorStyle::ResizeUpRightDownLeft,
        HKind::Corner(..) => CursorStyle::ResizeUpLeftDownRight,
        HKind::Radius | HKind::Scale | HKind::ArrowEnd(_) => CursorStyle::Crosshair,
        HKind::RotationSlide => if dragging { CursorStyle::ClosedHand } else { CursorStyle::OpenHand },
        HKind::Bend | HKind::ArrowLabel => CursorStyle::PointingHand,
    }
}

/// The handles: white squares (a blob's box), blue dots (a radius), the ↻
/// knob on a stem, a diamond (an arrow's bend), a dashed box (its label).
fn paint_handles(handles: &[Handle], scene: &Scene, xf: &Xf, window: &mut Window, cx: &mut App) {
    let blue = blue();
    let white: Hsla = gpui::rgb(0xFFFFFF).into();
    for h in handles {
        let c = xf.to_px(h.at);
        match h.kind {
            HKind::Corner(..) => {
                let s = px(super::handles::HANDLE_PX);
                window.paint_quad(quad(Bounds::new(point(c.x - s / 2.0, c.y - s / 2.0), size(s, s)), px(0.), white, px(1.), blue, Default::default()));
            }
            HKind::Radius | HKind::Scale | HKind::ArrowEnd(_) | HKind::RotationSlide => {
                let r = px(4.5);
                window.paint_quad(quad(Bounds::new(point(c.x - r, c.y - r), size(r * 2.0, r * 2.0)), r, blue, px(1.5), white, Default::default()));
            }
            HKind::Rotate => {
                // a stem down to the shape's outline
                if let Some(t) = super::handles::turned_outline(scene, &h.obj) {
                    // a turned shape: the box is its own frame's, so the stem meets its edge
                    let centre = xf.to_px(t.centre);
                    let (dx, dy) = (f32::from(c.x - centre.x), f32::from(c.y - centre.y));
                    let l = dx.hypot(dy).max(1.0);
                    let (ux, uy) = (dx / l, dy / l);
                    // into the box's frame (screen y runs down: turn by +angle)
                    let a = t.angle.to_radians();
                    let (lx, ly) = (ux * a.cos() - uy * a.sin(), ux * a.sin() + uy * a.cos());
                    let hw = t.half.x * xf.scale + 4.0;
                    let hh = t.half.y * xf.scale + 4.0;
                    let t_edge = (hw / lx.abs().max(1e-4)).min(hh / ly.abs().max(1e-4));
                    let foot = point(centre.x + px(ux * t_edge), centre.y + px(uy * t_edge));
                    let r = super::handles::KNOB_PX / 2.0;
                    let top = point(c.x - px(ux * r), c.y - px(uy * r));
                    if l > t_edge + r {
                        stroke_poly(&[foot, top], px(1.), with_alpha(blue, 0.8), None, window);
                    }
                } else if let Some(rect) = super::handles::outline(scene, &h.obj) {
                    let ob = xf.rect_px(&rect).dilate(px(4.));
                    let centre = ob.center();
                    let (dx, dy) = (f32::from(c.x - centre.x), f32::from(c.y - centre.y));
                    let l = dx.hypot(dy).max(1.0);
                    let (ux, uy) = (dx / l, dy / l);
                    // from the knob towards the centre until the box edge
                    let hw = f32::from(ob.size.width) / 2.0;
                    let hh = f32::from(ob.size.height) / 2.0;
                    let t_edge = (hw / ux.abs().max(1e-4)).min(hh / uy.abs().max(1e-4));
                    let foot = point(centre.x + px(ux * t_edge), centre.y + px(uy * t_edge));
                    let r = super::handles::KNOB_PX / 2.0;
                    let top = point(c.x - px(ux * r), c.y - px(uy * r));
                    if l > t_edge + r {
                        stroke_poly(&[foot, top], px(1.), with_alpha(blue, 0.8), None, window);
                    }
                }
                let r = px(super::handles::KNOB_PX / 2.0);
                window.paint_quad(quad(Bounds::new(point(c.x - r, c.y - r), size(r * 2.0, r * 2.0)), r, white, px(1.), blue, Default::default()));
                // ↻: an arc with a barb
                let rr = 4.6f32;
                let mut b = PathBuilder::stroke(px(1.3));
                let at = |deg: f32| point(c.x + px(rr * deg.to_radians().cos()), c.y - px(rr * deg.to_radians().sin()));
                b.move_to(at(120.0));
                for k in 1..=10 {
                    b.line_to(at(120.0 - k as f32 * 27.0));
                }
                if let Ok(p) = b.build() {
                    window.paint_path(p, blue);
                }
                let end = at(120.0 - 270.0);
                let mut b = PathBuilder::stroke(px(1.2));
                b.move_to(point(end.x - px(3.0), end.y - px(1.4)));
                b.line_to(end);
                b.line_to(point(end.x + px(0.2), end.y - px(3.2)));
                if let Ok(p) = b.build() {
                    window.paint_path(p, blue);
                }
            }
            HKind::Bend => {
                let r = 5.0f32;
                let mut b = PathBuilder::fill();
                b.move_to(point(c.x, c.y - px(r)));
                b.line_to(point(c.x + px(r), c.y));
                b.line_to(point(c.x, c.y + px(r)));
                b.line_to(point(c.x - px(r), c.y));
                b.close();
                if let Ok(p) = b.build() {
                    window.paint_path(p, blue);
                }
                let mut b = PathBuilder::stroke(px(1.));
                b.move_to(point(c.x, c.y - px(r + 0.5)));
                b.line_to(point(c.x + px(r + 0.5), c.y));
                b.line_to(point(c.x, c.y + px(r + 0.5)));
                b.line_to(point(c.x - px(r + 0.5), c.y));
                b.close();
                if let Ok(p) = b.build() {
                    window.paint_path(p, white);
                }
            }
            HKind::ArrowLabel => {
                let r = scene
                    .draws
                    .iter()
                    .find_map(|d| match d {
                        Draw::Label { obj, at, latex, size, math, .. } if *obj == h.obj => Some(super::handles::label_rect(*at, latex, *size, *math)),
                        _ => None,
                    })
                    .map(|r| xf.rect_px(&r).dilate(px(3.)))
                    .unwrap_or_else(|| Bounds::new(point(c.x - px(8.), c.y - px(8.)), size(px(16.), px(16.))));
                window.paint_quad(quad(r, px(0.), with_alpha(blue, 0.05), px(1.), blue, gpui::BorderStyle::Dashed));
            }
        }
    }
    let _ = cx;
}

/// The live readout by the pointer: a small boxed mono note ("45°").
fn paint_readout(text: &str, at: Point<Pixels>, bounds: Bounds<Pixels>, avoid: &[Bounds<Pixels>], window: &mut Window, cx: &mut App) {
    let t = cx.global::<crate::ui::widgets::theme::Theme>().clone();
    let fs = t.keycap_size;
    let run = TextRun { len: text.len(), font: t.mono(), color: t.ink, background_color: None, underline: None, strikethrough: None };
    let line = window.text_system().shape_line(text.to_string().into(), fs, &[run], None);
    let lh = fs * 1.3;
    let (w, h) = (line.width() + px(10.), lh + px(4.));
    // below right of the pointer, inside the canvas
    let fit = |x: Pixels, y: Pixels| -> (Pixels, Pixels) {
        let x = if x + w > bounds.right() - px(4.) { at.x - w - px(12.) } else { x };
        let y = if y + h > bounds.bottom() - px(4.) { at.y - h - px(12.) } else { y };
        (x, y)
    };
    let clear = |(x, y): (Pixels, Pixels)| {
        let b = Bounds::new(point(x - px(3.), y - px(3.)), size(w + px(6.), h + px(6.)));
        !avoid.iter().any(|a| a.intersects(&b))
    };
    // below right of the pointer; else the other corners, then further down
    let (dx, dy) = (px(16.), px(14.));
    let tries = [
        (at.x + dx, at.y + dy),
        (at.x + dx, at.y - h - px(12.)),
        (at.x - w - px(12.), at.y + dy),
        (at.x - w - px(12.), at.y - h - px(12.)),
        (at.x + dx, at.y + dy + h + px(8.)),
        (at.x + dx, at.y - h * 2.0 - px(20.)),
        (at.x + dx + px(40.), at.y + dy),
    ];
    let (mut x, mut y) = tries.iter().map(|&(x, y)| fit(x, y)).find(|&p| clear(p)).unwrap_or_else(|| fit(at.x + dx, at.y + dy));
    // whatever the pointer does, the note stays inside the canvas
    x = x.min(bounds.right() - w - px(4.)).max(bounds.left() + px(4.));
    // (clear of the zoom note in the lower right corner)
    y = y.min(bounds.bottom() - h - px(26.)).max(bounds.top() + px(4.));
    let b = Bounds::new(point(x, y), size(w, h));
    window.paint_quad(quad(b, px(2.), gpui::rgb(0xFFFFFF), px(1.), slate(), Default::default()));
    let _ = line.paint(point(x + px(5.), y + px(2.)), lh, TextAlign::Left, None, window, cx);
}

/// The figures' face for the draft.
pub fn charter(italic: bool) -> Font {
    Font {
        family: crate::ui::widgets::fonts::CHARTER.into(),
        features: Default::default(),
        fallbacks: Some(FontFallbacks::from_fonts(vec!["Latin Modern Math".into(), "Latin Modern Roman".into()])),
        weight: FontWeight::NORMAL,
        style: if italic { FontStyle::Italic } else { FontStyle::Normal },
    }
}

/// A label centred horizontally on `baseline.x`, its baseline at `baseline.y`.
/// Shape all runs together so centring and glyph positions use the full label.
pub fn paint_label(styled: &[(String, bool)], baseline: Point<Pixels>, fs: Pixels, colour: Hsla, window: &mut Window, cx: &mut App) {
    let text: String = styled.iter().map(|(text, _)| text.as_str()).collect();
    let runs: Vec<TextRun> = styled.iter().map(|(text, italic)| TextRun {
        len: text.len(), font: charter(*italic), color: colour,
        background_color: None, underline: None, strikethrough: None,
    }).collect();
    let line = window.text_system().shape_line(text.into(), fs, &runs, None);
    let lh = fs * 1.2;
    // GPUI sets the baseline at (lh - ascent - descent) / 2 + ascent
    let top = baseline.y - ((lh - line.ascent - line.descent) / 2.0 + line.ascent);
    let _ = line.paint(point(baseline.x - line.width() / 2.0, top), lh, TextAlign::Left, None, window, cx);
}
