//! (figure-qol class E) What the shorthand can say and the panel couldn't:
//! the shared fields of a multi-selection ("mixed" where the objects
//! differ), a position row for every placeable object (with a cells / cm
//! switch and an auto-place button), free text's colour and size, an
//! arrow's label position, a node's loop, a blob's parent.
//!
//! A child of `inspector` so it can use the panel's building blocks;
//! everything here is new, so the merge with the other panel work stays small.
use super::{HEADS, choice, chip, head_glyph, note, parse_num, fmt_num, row, stack, unit};
use super::{doc, handles};
use crate::ui::figure::interact::owner;
use crate::ui::figure::menu::record_bounds;
use crate::ui::figure::view::FigureView;
use crate::ui::widgets::theme::Theme;
use gpui::{AnyElement, Context, StatefulInteractiveElement as _, IntoElement, ParentElement as _, SharedString, Styled as _, Window, div, px};
use ntex_figures::model::*;

/// `Some(v)` when every value is `v`; None when they differ (or there are none).
fn shared<T: PartialEq>(mut vals: impl Iterator<Item = T>) -> Option<T> {
    let first = vals.next()?;
    vals.all(|v| v == first).then_some(first)
}

/// A header for the group of controls that apply to one kind of object.
fn heading(t: &Theme, s: String) -> AnyElement {
    div().pt(px(6.)).font(t.serif_italic()).text_size(t.small_size).text_color(t.slate).child(s).into_any_element()
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The written position of anything placeable (elements included).
fn pos_of(fig: &Figure, obj: &ObjRef) -> Option<Pos> {
    if let ObjRef::Element(e) = obj {
        return fig.items.iter().find_map(|it| match it {
            Item::Blob(b) => b.elements.iter().find(|x| &x.name == e).and_then(|x| x.pos),
            _ => None,
        });
    }
    crate::ui::figure::tools::item_pos(fig, obj)
}

impl FigureView {
    // ---- helpers ----------------------------------------------------------------

    /// The items of the selection (owners, once each): (index, item), only the kinds
    /// that have a panel of their own.
    fn picked(&self) -> Vec<(usize, Item)> {
        let mut seen: Vec<usize> = Vec::new();
        let mut out = Vec::new();
        for o in &self.selection {
            let o = owner(o);
            if !matches!(o, ObjRef::Node(_) | ObjRef::Arrow(_) | ObjRef::Blob(_) | ObjRef::NGon(_) | ObjRef::Cayley(_) | ObjRef::Text(_)) {
                continue;
            }
            if let Some(i) = handles::item_index(&self.doc.figure, &o)
                && !seen.contains(&i)
            {
                seen.push(i);
                out.push((i, self.doc.figure.items[i].clone()));
            }
        }
        out
    }

    /// Change every selected item that `f` changes: ONE undo step.
    fn edit_many(&mut self, what: &str, cx: &mut Context<Self>, f: impl Fn(&mut Item) -> bool) -> bool {
        let mut fig = self.doc.figure.clone();
        let mut changed = false;
        for (i, old) in self.picked() {
            let mut item = old.clone();
            if !f(&mut item) || item == old {
                continue;
            }
            match doc::apply(&mut fig, &Edit::SetItem { index: i, item: item.clone() }) {
                Ok(_) => {}
                Err(e) if e.contains("not implemented") || e.contains("not built") => fig.items[i] = item,
                Err(_) => continue,
            }
            changed = true;
        }
        changed && self.commit_grouped(fig, what, None, cx)
    }

    /// How many of the selected objects have a written position to clear.
    pub fn autoplace_targets(&self) -> usize {
        let mut n = 0;
        for o in &self.selection {
            let o = owner(o);
            if matches!(o, ObjRef::Node(_) | ObjRef::Blob(_) | ObjRef::NGon(_) | ObjRef::Cayley(_) | ObjRef::Element(_)) && pos_of(&self.doc.figure, &o).is_some() {
                n += 1;
            }
        }
        n
    }

    /// Auto-place: clear the written position of every selected object that has
    /// one, so layout places it again (nodes by their arrows, shapes in a row).
    /// One undo step.
    pub fn auto_place(&mut self, cx: &mut Context<Self>) -> bool {
        let mut fig = self.doc.figure.clone();
        let mut done = 0;
        for o in self.selection.clone() {
            let o = owner(&o);
            let Some(i) = handles::item_index(&fig, &o) else { continue };
            let mut item = fig.items[i].clone();
            let did = match (&mut item, &o) {
                (Item::Node(n), _) => n.pos.take().is_some(),
                (Item::Blob(b), ObjRef::Blob(_)) => b.pos.take().is_some(),
                (Item::NGon(g), _) => g.pos.take().is_some(),
                (Item::Cayley(c), _) => c.pos.take().is_some(),
                (Item::Blob(b), ObjRef::Element(e)) => b.elements.iter_mut().find(|x| &x.name == e).is_some_and(|x| x.pos.take().is_some()),
                _ => false,
            };
            if !did {
                continue;
            }
            match doc::apply(&mut fig, &Edit::SetItem { index: i, item: item.clone() }) {
                Ok(_) => {}
                Err(_) => fig.items[i] = item,
            }
            done += 1;
        }
        if done == 0 {
            return false;
        }
        let ok = self.commit_grouped(fig, "auto-place", None, cx);
        if ok {
            self.toast(if done == 1 { "Auto-placed — ⌘Z puts it back.".to_string() } else { format!("Auto-placed {done} — ⌘Z puts them back.") }, cx);
        }
        ok
    }

    // ---- one object: position ---------------------------------------------------

    /// The "At" row for a node, blob, element, polygon, Cayley graph or text:
    /// two fields (cell or cm), a cells / cm switch, and Auto-place. `key` makes the ids.
    pub(super) fn pos_rows(&mut self, t: &Theme, key: &str, obj: &ObjRef, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let fig = self.doc.figure.clone();
        let pos = pos_of(&fig, obj);
        let abs = matches!(pos, Some(Pos::Abs(..)));
        let (a, b) = match pos {
            Some(Pos::Cell(c, r)) => (fmt_num(c), fmt_num(r)),
            Some(Pos::Abs(x, y)) => (fmt_num(x), fmt_num(y)),
            None => (String::new(), String::new()),
        };
        let part = |v: &mut FigureView, which: usize, s: &str, o: &ObjRef, cx: &mut Context<FigureView>| {
            let Some(x) = parse_num(s) else { return };
            let cur = pos_of(&v.doc.figure, o).or_else(|| {
                let p = crate::ui::figure::geom::center_of(&v.doc.scene, o)?;
                // where a drag would pin it: blobs, elements and text in cm, the rest on cells
                Some(match o {
                    ObjRef::Blob(_) | ObjRef::Text(_) | ObjRef::Element(_) => {
                        let k = if v.doc.scene.scale > 0.0 { v.doc.scene.scale } else { 1.0 };
                        crate::ui::figure::geom::abs(Pt { x: p.x / k, y: p.y / k })
                    }
                    _ => crate::ui::figure::geom::snap(p, v.cell(), true),
                })
            });
            let pos = match cur {
                Some(Pos::Abs(p, q)) => if which == 0 { Pos::Abs(x, q) } else { Pos::Abs(p, x) },
                Some(Pos::Cell(p, q)) => if which == 0 { Pos::Cell(x, q) } else { Pos::Cell(p, x) },
                None => if which == 0 { Pos::Cell(x, 0.0) } else { Pos::Cell(0.0, x) },
            };
            let mut fig = v.doc.figure.clone();
            crate::ui::figure::tools::move_obj(&mut fig, o, pos);
            v.commit_grouped(fig, "position", Some(format!("field:pos:{o:?}")), cx);
        };
        let (o1, o2) = (obj.clone(), obj.clone());
        let fa = self.field(&format!("{key}.col"), &a, true, Some(40.), "auto", window, cx, move |v, s, _, cx| part(v, 0, s, &o1, cx));
        let fb = self.field(&format!("{key}.row"), &b, true, Some(40.), "auto", window, cx, move |v, s, _, cx| part(v, 1, s, &o2, cx));
        let o = obj.clone();
        let spacing = fig.globals.spacing;
        let switch = self.button(&format!("{key}.unit"), if abs { "cm" } else { "cells" }, cx, move |v, _, cx| {
            let Some(p) = pos_of(&v.doc.figure, &o) else { return };
            let q = match p {
                Pos::Cell(c, r) => Pos::Abs(round2(c * spacing.0), round2(-r * spacing.1)),
                Pos::Abs(x, y) => Pos::Cell(round2(x / spacing.0), round2(-y / spacing.1)),
            };
            let mut fig = v.doc.figure.clone();
            crate::ui::figure::tools::move_obj(&mut fig, &o, q);
            v.commit_grouped(fig, "position units", None, cx);
        });
        let mut line = div().flex().flex_row().items_center().gap(px(4.)).child(fa).child(unit(t, ",")).child(fb);
        if pos.is_some() {
            line = line.child(switch);
        }
        let mut out = vec![row(t, "At", line).into_any_element()];
        if !matches!(obj, ObjRef::Text(_)) {
            let auto = self.button(&format!("{key}.autoplace"), "Auto-place", cx, |v, _, cx| {
                v.auto_place(cx);
            });
            let says = if pos.is_some() { "clears `at`: layout places it" } else { "placed by layout (drag it to pin it)" };
            out.push(row(t, "", div().flex().flex_row().flex_wrap().items_center().gap(px(8.)).when_pos(pos.is_some(), auto).child(note(t, says))).into_any_element());
        }
        out
    }

    /// A node's "Add loop arrow" (an arrow from it to itself).
    pub(super) fn loop_row(&mut self, t: &Theme, name: &str, cx: &mut Context<Self>) -> AnyElement {
        let n = name.to_string();
        let b = self.button(&format!("node.{name}.loop"), "Add loop arrow", cx, move |v, w, cx| v.add_arrow(n.clone(), n.clone(), w, cx));
        row(t, "Loop", b).into_any_element()
    }

    // ---- free text: colour and size ----------------------------------------------

    pub(super) fn text_rows(&mut self, t: &Theme, i: usize, x: &Text, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let colours = self.colours(t, cx);
        let sizes = ["tiny", "small", "normal", "large", "huge"];
        let cur = sizes.iter().position(|s| Some(*s) == x.size.as_deref().or(Some("normal")));
        let xs = x.clone();
        let go = self.on2(cx, move |v, k, cx| {
            let mut x = xs.clone();
            x.size = if sizes[k] == "normal" { None } else { Some(sizes[k].to_string()) };
            v.set_item(i, Item::Text(x), "text size", None, vec![], cx);
        });
        vec![stack(t, "Colour", colours).into_any_element(), row(t, "Size", choice(t, "panel.textsize", &sizes, cur, &self.ui_bounds, go)).into_any_element()]
    }

    // ---- an arrow: where its label sits -------------------------------------------

    pub(super) fn label_pos_row(&mut self, t: &Theme, i: usize, a: &Arrow, cx: &mut Context<Self>) -> AnyElement {
        let opts = ["start", "middle", "end"];
        let cur = [LabelPos::NearStart, LabelPos::Middle, LabelPos::NearEnd].iter().position(|p| *p == a.label_pos);
        let al = a.clone();
        let go = self.on2(cx, move |v, k, cx| {
            let mut a = al.clone();
            a.label_pos = [LabelPos::NearStart, LabelPos::Middle, LabelPos::NearEnd][k];
            v.set_item(i, Item::Arrow(a), "label position", None, vec![], cx);
        });
        row(t, "Label at", choice(t, "panel.labelpos", &opts, cur, &self.ui_bounds, go)).into_any_element()
    }

    // ---- a blob inside another ------------------------------------------------------

    pub(super) fn parent_row(&mut self, t: &Theme, i: usize, b: &Blob, cx: &mut Context<Self>) -> Option<AnyElement> {
        let fig = &self.doc.figure;
        let others: Vec<String> = fig
            .items
            .iter()
            .filter_map(|it| match it {
                Item::Blob(o) if o.name != b.name && !inside(fig, o, &b.name) => Some(o.name.clone()),
                _ => None,
            })
            .collect();
        if others.is_empty() && b.parent.is_none() {
            return None;
        }
        let mut opts: Vec<String> = vec!["none".into()];
        opts.extend(others);
        let cur = Some(b.parent.as_ref().and_then(|p| opts.iter().position(|o| o == p)).unwrap_or(0));
        let names = opts.clone();
        let bb = b.clone();
        let go = self.on2(cx, move |v, k, cx| {
            let mut b = bb.clone();
            b.parent = if k == 0 { None } else { Some(names[k].clone()) };
            v.set_item(i, Item::Blob(b), "parent", None, vec![], cx);
        });
        let refs: Vec<&str> = opts.iter().map(|s| s.as_str()).collect();
        Some(row(t, "Inside", choice(t, "panel.parent", &refs, cur, &self.ui_bounds, go)).into_any_element())
    }

    // ---- many objects -------------------------------------------------------------

    /// The controls every selected object of a kind shares, a group per kind;
    /// a value the objects disagree on shows as "mixed" and the first click sets it for all.
    pub(super) fn shared_sections(&mut self, t: &Theme, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let items = self.picked();
        let arrows: Vec<Arrow> = items.iter().filter_map(|(_, it)| if let Item::Arrow(a) = it { Some(a.clone()) } else { None }).collect();
        let blobs: Vec<Blob> = items.iter().filter_map(|(_, it)| if let Item::Blob(b) = it { Some(b.clone()) } else { None }).collect();
        let ngons: Vec<NGon> = items.iter().filter_map(|(_, it)| if let Item::NGon(g) = it { Some(g.clone()) } else { None }).collect();
        let cayleys: Vec<Cayley> = items.iter().filter_map(|(_, it)| if let Item::Cayley(c) = it { Some(c.clone()) } else { None }).collect();
        let texts: Vec<Text> = items.iter().filter_map(|(_, it)| if let Item::Text(x) = it { Some(x.clone()) } else { None }).collect();
        let mut out: Vec<AnyElement> = Vec::new();
        let mixed_note = |t: &Theme, on: bool| note(t, if on { "mixed: a click sets all" } else { "" }).into_any_element();

        if !arrows.is_empty() {
            out.push(heading(t, plural(arrows.len(), "arrow", "arrows")));
            // head
            let cur = shared(arrows.iter().map(|a| a.head));
            let mut heads = div().flex().flex_row().flex_wrap().gap(px(3.));
            for h in HEADS {
                let go = self.on(cx, move |v, _, cx| {
                    v.edit_many("head", cx, |it| {
                        if let Item::Arrow(a) = it {
                            a.head = h;
                        }
                        true
                    });
                });
                heads = heads.child(
                    div()
                        .relative()
                        .flex_none()
                        .child(chip(t, SharedString::from(format!("many.head.{h:?}")), head_glyph(h), cur == Some(h), false).font(t.math_font()).min_w(px(28.)).on_click(move |_, w, cx| go(w, cx)))
                        .child(record_bounds(self.ui_bounds.clone(), format!("many.head.{h:?}"))),
                );
            }
            out.push(row(t, "Head", heads).into_any_element());
            if cur.is_none() {
                out.push(row(t, "", mixed_note(t, true)).into_any_element());
            }
            // line
            let lines = ["solid", "dashed", "dotted"];
            let all = [Line::Solid, Line::Dashed, Line::Dotted];
            let cur = shared(arrows.iter().map(|a| a.line)).and_then(|l| all.iter().position(|x| *x == l));
            let go = self.on2(cx, move |v, k, cx| {
                v.edit_many("line", cx, |it| {
                    if let Item::Arrow(a) = it {
                        a.line = all[k];
                    }
                    true
                });
            });
            out.push(row(t, "Line", choice(t, "many.line", &lines, cur, &self.ui_bounds, go)).into_any_element());
            if cur.is_none() {
                out.push(row(t, "", mixed_note(t, true)).into_any_element());
            }
            // label position
            let pos = ["start", "middle", "end"];
            let allp = [LabelPos::NearStart, LabelPos::Middle, LabelPos::NearEnd];
            let cur = shared(arrows.iter().map(|a| a.label_pos)).and_then(|l| allp.iter().position(|x| *x == l));
            let go = self.on2(cx, move |v, k, cx| {
                v.edit_many("label position", cx, |it| {
                    if let Item::Arrow(a) = it {
                        a.label_pos = allp[k];
                    }
                    true
                });
            });
            out.push(row(t, "Label at", choice(t, "many.labelpos", &pos, cur, &self.ui_bounds, go)).into_any_element());
            if cur.is_none() {
                out.push(row(t, "", mixed_note(t, true)).into_any_element());
            }
            // bend
            let bends = ["straight", "left", "right"];
            let kind = |b: f32| if b == 0.0 { Some(0) } else if b == 30.0 { Some(1) } else if b == -30.0 { Some(2) } else { None };
            let cur = shared(arrows.iter().map(|a| kind(a.bend))).flatten();
            let go = self.on2(cx, move |v, k, cx| {
                v.edit_many("bend", cx, |it| {
                    if let Item::Arrow(a) = it
                        && !a.is_loop
                    {
                        a.bend = [0.0, 30.0, -30.0][k];
                    }
                    true
                });
            });
            out.push(row(t, "Bend", choice(t, "many.bend", &bends, cur, &self.ui_bounds, go)).into_any_element());
            // swap / iso / thick
            let tri = |this: &FigureView, id: &str, label: &str, vals: Vec<bool>, cx: &mut Context<FigureView>, set: fn(&mut Arrow, bool)| {
                let all_on = vals.iter().all(|b| *b);
                let any_on = vals.iter().any(|b| *b);
                let text = if any_on && !all_on { format!("{label} (mixed)") } else { label.to_string() };
                let what = label.to_string();
                this.check(&format!("many.{id}"), &text, all_on, cx, move |v, on, cx| {
                    v.edit_many(&what, cx, |it| {
                        if let Item::Arrow(a) = it {
                            set(a, on);
                        }
                        true
                    });
                })
            };
            let swap = tri(self, "swap", "Swap side", arrows.iter().map(|a| a.side == LabelSide::Swap).collect(), cx, |a, on| a.side = if on { LabelSide::Swap } else { LabelSide::Auto });
            let iso = tri(self, "iso", "≅", arrows.iter().map(|a| a.iso).collect(), cx, |a, on| a.iso = on);
            let thick = tri(self, "thick", "Thick", arrows.iter().map(|a| a.thick).collect(), cx, |a, on| a.thick = on);
            out.push(row(t, "", div().flex().flex_row().flex_wrap().gap_x(px(12.)).child(swap).child(iso).child(thick)).into_any_element());
            let rev = self.button("many.reverse", "Reverse", cx, |v, _, cx| {
                v.reverse_selection(cx);
            });
            out.push(row(t, "Ends", div().flex().flex_row().items_center().gap(px(8.)).child(rev).child(note(t, "⌘R"))).into_any_element());
        }

        if !blobs.is_empty() {
            out.push(heading(t, plural(blobs.len(), "blob", "blobs")));
            let tri = |this: &FigureView, id: &str, label: &str, vals: Vec<bool>, cx: &mut Context<FigureView>, set: fn(&mut Blob, bool)| {
                let all_on = vals.iter().all(|b| *b);
                let text = if vals.iter().any(|b| *b) && !all_on { format!("{label} (mixed)") } else { label.to_string() };
                let what = label.to_lowercase();
                this.check(&format!("many.blob.{id}"), &text, all_on, cx, move |v, on, cx| {
                    v.edit_many(&what, cx, |it| {
                        if let Item::Blob(b) = it {
                            set(b, on);
                        }
                        true
                    });
                })
            };
            let shaded = tri(self, "shaded", "Shaded", blobs.iter().map(|b| b.shaded).collect(), cx, |b, on| b.shaded = on);
            out.push(row(t, "", shaded).into_any_element());
            let shapes = ["round", "tall", "wide", "bean"];
            let alls = [BlobShape::Round, BlobShape::Tall, BlobShape::Wide, BlobShape::Bean];
            let cur = shared(blobs.iter().map(|b| b.shape)).and_then(|s| alls.iter().position(|x| *x == s));
            let go = self.on2(cx, move |v, k, cx| {
                v.edit_many("shape", cx, |it| {
                    if let Item::Blob(b) = it {
                        b.shape = alls[k];
                    }
                    true
                });
            });
            out.push(row(t, "Shape", choice(t, "many.shape", &shapes, cur, &self.ui_bounds, go)).into_any_element());
            let places = ["above", "below", "left", "right", "inside"];
            let allp = [LabelPlace::Above, LabelPlace::Below, LabelPlace::Left, LabelPlace::Right, LabelPlace::Inside];
            let cur = shared(blobs.iter().map(|b| b.label_place)).and_then(|s| allp.iter().position(|x| *x == s));
            let go = self.on2(cx, move |v, k, cx| {
                v.edit_many("label place", cx, |it| {
                    if let Item::Blob(b) = it {
                        b.label_place = allp[k];
                    }
                    true
                });
            });
            out.push(row(t, "Place", choice(t, "many.place", &places, cur, &self.ui_bounds, go)).into_any_element());
        }

        if !ngons.is_empty() {
            out.push(heading(t, plural(ngons.len(), "n-gon", "n-gons")));
            let tri = |this: &FigureView, id: &str, label: &str, vals: Vec<bool>, cx: &mut Context<FigureView>, set: fn(&mut NGon, bool)| {
                let all_on = vals.iter().all(|b| *b);
                let text = if vals.iter().any(|b| *b) && !all_on { format!("{label} (mixed)") } else { label.to_string() };
                let what = label.to_lowercase();
                this.check(&format!("many.ngon.{id}"), &text, all_on, cx, move |v, on, cx| {
                    v.edit_many(&what, cx, |it| {
                        if let Item::NGon(g) = it {
                            set(g, on);
                        }
                        true
                    });
                })
            };
            let fill = tri(self, "fill", "Fill", ngons.iter().map(|g| g.shaded).collect(), cx, |g, on| g.shaded = on);
            let labels = tri(self, "labels", "Labels", ngons.iter().map(|g| g.show_labels).collect(), cx, |g, on| g.show_labels = on);
            let axes = tri(self, "axes", "All axes", ngons.iter().map(|g| g.all_axes).collect(), cx, |g, on| {
                g.all_axes = on;
                if on {
                    g.axes.clear();
                }
            });
            let rot = tri(self, "rotation", "Rotation arrow", ngons.iter().map(|g| g.rotation > 0).collect(), cx, |g, on| g.rotation = if on { g.rotation.max(1) } else { 0 });
            out.push(row(t, "", div().flex().flex_row().flex_wrap().gap_x(px(12.)).child(fill).child(labels)).into_any_element());
            out.push(row(t, "", div().flex().flex_row().flex_wrap().gap_x(px(12.)).child(axes).child(rot)).into_any_element());
        }

        if !cayleys.is_empty() {
            out.push(heading(t, plural(cayleys.len(), "Cayley graph", "Cayley graphs")));
            let tri = |this: &FigureView, id: &str, label: &str, vals: Vec<bool>, cx: &mut Context<FigureView>, set: fn(&mut Cayley, bool)| {
                let all_on = vals.iter().all(|b| *b);
                let text = if vals.iter().any(|b| *b) && !all_on { format!("{label} (mixed)") } else { label.to_string() };
                let what = label.to_lowercase();
                this.check(&format!("many.cayley.{id}"), &text, all_on, cx, move |v, on, cx| {
                    v.edit_many(&what, cx, |it| {
                        if let Item::Cayley(c) = it {
                            set(c, on);
                        }
                        true
                    });
                })
            };
            let labels = tri(self, "labels", "Labels", cayleys.iter().map(|c| c.labels).collect(), cx, |c, on| c.labels = on);
            let legend = tri(self, "legend", "Legend", cayleys.iter().map(|c| c.legend).collect(), cx, |c, on| c.legend = on);
            out.push(row(t, "", div().flex().flex_row().flex_wrap().gap_x(px(12.)).child(labels).child(legend)).into_any_element());
            let names = ["auto", "circle", "prism", "grid"];
            let alll = [CayleyLayout::Auto, CayleyLayout::Circle, CayleyLayout::Prism, CayleyLayout::Grid];
            let cur = shared(cayleys.iter().map(|c| c.layout)).and_then(|s| alll.iter().position(|x| *x == s));
            let go = self.on2(cx, move |v, k, cx| {
                v.edit_many("layout", cx, |it| {
                    if let Item::Cayley(c) = it {
                        c.layout = alll[k];
                    }
                    true
                });
            });
            out.push(row(t, "Layout", choice(t, "many.layout", &names, cur, &self.ui_bounds, go)).into_any_element());
        }

        if !texts.is_empty() {
            out.push(heading(t, plural(texts.len(), "text", "texts")));
            let sizes = ["tiny", "small", "normal", "large", "huge"];
            let cur = shared(texts.iter().map(|x| x.size.clone().unwrap_or_else(|| "normal".into()))).and_then(|s| sizes.iter().position(|x| *x == s));
            let go = self.on2(cx, move |v, k, cx| {
                v.edit_many("text size", cx, |it| {
                    if let Item::Text(x) = it {
                        x.size = if sizes[k] == "normal" { None } else { Some(sizes[k].to_string()) };
                    }
                    true
                });
            });
            out.push(row(t, "Size", choice(t, "many.textsize", &sizes, cur, &self.ui_bounds, go)).into_any_element());
        }

        // placed things: Auto-place for all of them at once
        if self.autoplace_targets() > 0 {
            let auto = self.button("many.autoplace", "Auto-place", cx, |v, _, cx| {
                v.auto_place(cx);
            });
            out.push(row(t, "", div().flex().flex_row().items_center().gap(px(8.)).child(auto).child(note(t, "clears `at`"))).into_any_element());
        }
        out
    }
}

fn round2(v: f32) -> f32 {
    (v * 100.0).round() / 100.0
}

/// Is `b` inside the blob called `name` (at any depth)?
fn inside(fig: &Figure, b: &Blob, name: &str) -> bool {
    let mut cur = b.parent.clone();
    let mut guard = 0;
    while let Some(p) = cur {
        if p == name {
            return true;
        }
        cur = match fig.items.iter().find_map(|it| match it {
            Item::Blob(x) if x.name == p => Some(x.parent.clone()),
            _ => None,
        }) {
            Some(next) => next,
            None => return false,
        };
        guard += 1;
        if guard > 100 {
            break;
        }
    }
    false
}

trait WhenPos: Sized {
    fn when_pos(self, on: bool, child: AnyElement) -> Self;
}
impl WhenPos for gpui::Div {
    fn when_pos(self, on: bool, child: AnyElement) -> Self {
        if on { self.child(child) } else { self }
    }
}
