//! (figure-qol class C) Editing labels after placing them: what the field
//! holds (maths by default, `$…$` hidden; a `text` chip for text mode), where
//! it sits (beside the label, not over it), the reading-order walk of Tab,
//! and the labels that had no way to be edited (Cayley vertex names, the
//! rotation arrow's `r`, an axis's label).
use super::geom::Xf;
use super::handles;
use super::interact::in_rect;
use super::view::FigureView;
use gpui::{Context, Pixels, Point, Window, point, px};
use ntex_figures::labeltext::{edit_form, label_from_edit_spelling};
use ntex_figures::model::*;

/// The object whose label a drawn label is edited as. The rotation arrow's
/// label is drawn under the rotation arrow (`NGonRotation`, class D); legends
/// and other labels have none.
pub fn edit_target(obj: &ObjRef) -> Option<ObjRef> {
    match obj {
        ObjRef::Node(_) | ObjRef::Arrow(_) | ObjRef::Blob(_) | ObjRef::Element(_) | ObjRef::NGonVertex(..) | ObjRef::CayleyVertex(..) | ObjRef::NGonAxis(..) | ObjRef::Text(_) => Some(obj.clone()),
        ObjRef::NGon(n) | ObjRef::NGonRotation(n) => Some(ObjRef::NGonRotationLabel(n.clone())),
        _ => None,
    }
}

/// The label draw that belongs to an edit target.
fn drawn_obj(target: &ObjRef) -> ObjRef {
    match target {
        ObjRef::NGonRotationLabel(n) => ObjRef::NGonRotation(n.clone()),
        o => o.clone(),
    }
}

/// What selecting `target` means: an axis label selects its n-gon, the
/// rotation arrow's label the rotation arrow (a part of its own).
pub fn selection_for(target: &ObjRef) -> ObjRef {
    match target {
        ObjRef::NGonAxis(n, _) => ObjRef::NGon(n.clone()),
        ObjRef::NGonRotationLabel(n) => ObjRef::NGonRotation(n.clone()),
        o => o.clone(),
    }
}

/// Is this one of the labels that only have an edit target (not an object of
/// their own: a click on it selects the n-gon)?
pub fn is_part_label(target: &ObjRef) -> bool {
    matches!(target, ObjRef::NGonAxis(..) | ObjRef::NGonRotationLabel(_))
}

impl FigureView {
    /// The part label (axis, rotation arrow's) under `p`, as an edit target.
    pub fn part_label_at(&self, p: Pt, xf: &Xf) -> Option<ObjRef> {
        let slack = 3.0 / xf.scale.max(1.0);
        let mut best: Option<(f32, ObjRef)> = None;
        for d in &self.doc.scene.draws {
            if let Draw::Label { obj, at, latex, size, math, .. } = d
                && let Some(t) = edit_target(obj).filter(is_part_label)
            {
                let r = handles::label_rect(*at, latex, *size, *math);
                if in_rect(p, &r, slack) {
                    let area = (r.max.x - r.min.x) * (r.max.y - r.min.y);
                    if best.as_ref().is_none_or(|(a, _)| area < *a) {
                        best = Some((area, t));
                    }
                }
            }
        }
        best.map(|b| b.1)
    }

    /// The label drawn at `p` as an edit target, if any (any kind).
    pub fn label_at(&self, p: Pt, xf: &Xf) -> Option<ObjRef> {
        let slack = 3.0 / xf.scale.max(1.0);
        let mut best: Option<(f32, ObjRef)> = None;
        for d in &self.doc.scene.draws {
            if let Draw::Label { obj, at, latex, size, math, .. } = d
                && let Some(t) = edit_target(obj)
            {
                let r = handles::label_rect(*at, latex, *size, *math);
                if in_rect(p, &r, slack) {
                    let area = (r.max.x - r.min.x) * (r.max.y - r.min.y);
                    if best.as_ref().is_none_or(|(a, _)| area < *a) {
                        best = Some((area, t));
                    }
                }
            }
        }
        best.map(|b| b.1)
    }

    /// Where the label of `target` is drawn (cm), if it is.
    pub fn label_spot(&self, target: &ObjRef) -> Option<Pt> {
        let want = drawn_obj(target);
        self.doc.scene.draws.iter().find_map(|d| match d {
            Draw::Label { obj, at, .. } if *obj == want => Some(*at),
            _ => None,
        })
    }

    /// The label's text as she wrote it, and whether it is maths (the
    /// field's starting state).
    pub fn label_start(&self, target: &ObjRef, raw: &str) -> (String, bool) {
        if matches!(target, ObjRef::Text(_)) {
            return (raw.to_string(), false);
        }
        edit_form(raw)
    }

    /// The model's label for what the in-place field holds now.
    pub fn field_label(&self, target: &ObjRef, cx: &gpui::App) -> String {
        let f = self.label_field.read(cx);
        if matches!(target, ObjRef::Text(_)) {
            f.text().to_string()
        } else {
            label_from_edit_spelling(f.text().trim(), f.math())
        }
    }

    /// Every label in the figure, in reading order: top to bottom, then left
    /// to right (labels within a line's height share a line). The edit targets.
    pub fn label_order(&self, include: Option<(&ObjRef, Pt)>) -> Vec<ObjRef> {
        let mut v: Vec<(ObjRef, Pt)> = Vec::new();
        // the label being edited is in the walk even while it is empty
        if let Some((o, at)) = include {
            v.push((o.clone(), at));
        }
        for d in &self.doc.scene.draws {
            if let Draw::Label { obj, at, latex, .. } = d
                && let Some(t) = edit_target(obj)
                && !latex.trim().is_empty()
                && !v.iter().any(|(o, _)| *o == t)
            {
                v.push((t, *at));
            }
        }
        v.sort_by(|a, b| b.1.y.total_cmp(&a.1.y));
        let tol = (self.cell().1 * 0.3).clamp(0.15, 0.5);
        let mut rows: Vec<Vec<(ObjRef, Pt)>> = Vec::new();
        for item in v {
            match rows.last_mut() {
                Some(row) if row[0].1.y - item.1.y < tol => row.push(item),
                _ => rows.push(vec![item]),
            }
        }
        rows.into_iter()
            .flat_map(|mut r| {
                r.sort_by(|a, b| a.1.x.total_cmp(&b.1.x));
                r
            })
            .map(|(o, _)| o)
            .collect()
    }

    /// Tab / ⇧Tab from the label being edited: the next label in reading order.
    pub fn neighbour_label(&self, cur: &ObjRef, at: Pt, dir: i32) -> Option<ObjRef> {
        let order = self.label_order(Some((cur, at)));
        if order.is_empty() {
            return None;
        }
        let at = order.iter().position(|o| o == cur)?;
        Some(order[(at as i32 + dir).rem_euclid(order.len() as i32) as usize].clone())
    }

    /// Where the in-place field's centre goes (window px, in the canvas's
    /// frame) for a label: above it, or, for an arrow, on the far side of its line.
    pub fn field_centre(&self, at: Pt, away: Option<Pt>, xf: &Xf, h: f32) -> Point<Pixels> {
        let p = xf.to_px(at);
        match away {
            // across the line: from the line's middle, away from the label
            Some(mid) => {
                let m = xf.to_px(mid);
                let (dx, dy) = (f32::from(m.x - p.x), f32::from(m.y - p.y));
                let l = dx.hypot(dy).max(1.0);
                let k = h * 0.5 + 8.0;
                point(m.x + px(dx / l * k), m.y + px(dy / l * k))
            }
            None => point(p.x, p.y - px(h * 0.5 + 14.0)),
        }
    }

    // ---- model edits for the labels that had none ----------------------------------

    /// Set the rotation / axis / Cayley-vertex label of `obj` to `typed`
    /// (already LaTeX or Unicode). None: not one of those.
    pub fn set_label_part(&mut self, obj: &ObjRef, typed: &str, group: Option<String>, cx: &mut Context<Self>) -> Option<bool> {
        let (item_obj, key) = match obj {
            ObjRef::NGonRotationLabel(n) => (ObjRef::NGon(n.clone()), format!("ngon:{n}.rot")),
            ObjRef::NGonAxis(n, c) => (ObjRef::NGon(n.clone()), format!("ngon:{n}.axis.{c}")),
            ObjRef::CayleyVertex(c, w) => (ObjRef::Cayley(c.clone()), format!("cayley:{c}.v.{w}")),
            _ => return None,
        };
        let i = handles::item_index(&self.doc.figure, &item_obj)?;
        let mut item = self.doc.figure.items[i].clone();
        let latex = if typed.trim().is_empty() { String::new() } else { super::doc::to_latex(typed.trim()) };
        match (&mut item, obj) {
            (Item::NGon(g), ObjRef::NGonRotationLabel(_)) => {
                g.rotation_label = (!latex.is_empty() && latex != ntex_figures::layout::ngon_rotation_default(g)).then(|| latex.clone());
            }
            (Item::NGon(g), ObjRef::NGonAxis(_, code)) => {
                g.axis_labels.retain(|(c, _)| c != code);
                if !latex.is_empty() {
                    g.axis_labels.push((*code, latex.clone()));
                }
            }
            (Item::Cayley(c), ObjRef::CayleyVertex(_, w)) => {
                c.vertex_names.retain(|(x, _)| x != w);
                if !latex.is_empty() && latex != *w {
                    c.vertex_names.push((w.clone(), latex.clone()));
                }
            }
            _ => return Some(false),
        }
        let spell = if latex.is_empty() { vec![] } else { vec![(key, latex, typed.trim().to_string())] };
        Some(self.set_item(i, item, "label", group, spell, cx))
    }

    /// The same edit on a draft figure (the live preview while typing).
    pub fn set_label_part_direct(fig: &mut Figure, obj: &ObjRef, typed: &str) -> bool {
        let latex = if typed.trim().is_empty() { String::new() } else { super::doc::to_latex(typed.trim()) };
        for it in fig.items.iter_mut() {
            match (it, obj) {
                (Item::NGon(g), ObjRef::NGonRotationLabel(n)) if &g.name == n => {
                    g.rotation_label = (!latex.is_empty()).then(|| latex.clone());
                    return true;
                }
                (Item::NGon(g), ObjRef::NGonAxis(n, code)) if &g.name == n => {
                    g.axis_labels.retain(|(c, _)| c != code);
                    if !latex.is_empty() {
                        g.axis_labels.push((*code, latex.clone()));
                    }
                    return true;
                }
                (Item::Cayley(c), ObjRef::CayleyVertex(n, w)) if &c.name == n => {
                    c.vertex_names.retain(|(x, _)| x != w);
                    if !latex.is_empty() {
                        c.vertex_names.push((w.clone(), latex.clone()));
                    }
                    return true;
                }
                _ => {}
            }
        }
        false
    }

    /// What the model has for a part label (LaTeX, as stored), or its default.
    pub fn part_label_text(&self, obj: &ObjRef) -> Option<String> {
        ntex_figures::spelling::as_typed(&self.doc.figure, obj)
    }

    /// A click on the `text` chip: this label's mode flips (one change).
    pub fn toggle_label_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(le) = self.label_edit.as_ref() else { return };
        if matches!(le.obj, ObjRef::Text(_)) {
            return;
        }
        let math = self.label_field.read(cx).math();
        self.label_field.update(cx, |f, cx| f.set_math(!math, cx));
        self.label_field.read(cx).focus.clone().focus(window, cx);
        self.preview_label(cx);
    }

    /// The canvas draws the label as it is typed.
    pub fn preview_label(&mut self, cx: &mut Context<Self>) {
        if let Some(le) = self.label_edit.clone() {
            let mut fig = self.doc.figure.clone();
            let label = self.field_label(&le.obj, cx);
            super::view::set_label_direct(&mut fig, &le.obj, &label);
            self.label_preview = Some(std::sync::Arc::new(super::doc::scene_of(&fig)));
            cx.notify();
        }
    }
}
