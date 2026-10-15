//! FIG-MODEL (interaction pass, FIGURES.md "Direct manipulation"): the new
//! shorthand options (`angle=`, `labels=none`, hidden labels, `plain`,
//! `color=` on n-gons), their layout and TikZ, the Scene's shape frames, and
//! the edits SetItem / MoveObj / Duplicate. Every edit is checked as
//! edit → format → parse == the edited model.
mod core_a_common;
use core_a_common::*;
use ntex_figures::layout::{cayley_radius_for, layout, ngon_label_index};
use ntex_figures::spelling::as_typed;

fn ed(fig: &mut Figure, edit: Edit) -> (Result<Option<String>, String>, String) {
    let before = fig.clone();
    let r = apply(fig, &edit);
    if r.is_err() {
        assert_eq!(*fig, before, "a refused edit changed the figure: {edit:?}");
    }
    let out = format(fig);
    let (g, _) = parse(&out);
    assert_eq!(without_spans(&g), without_spans(fig), "after {edit:?}\n--- formatted:\n{out}");
    assert_eq!(format(&g), out, "format not idempotent after {edit:?}");
    (r, out)
}

fn fig(src: &str) -> Figure {
    let (f, d) = p(src);
    no_errors(&d);
    f
}

fn fmt(src: &str) -> String {
    format(&fig(src))
}

fn ngon_of<'a>(f: &'a Figure, name: &str) -> &'a NGon {
    ngons(f).into_iter().find(|g| g.name == name).unwrap()
}

fn index_of(f: &Figure, pred: impl Fn(&Item) -> bool) -> usize {
    f.items.iter().position(pred).unwrap()
}

fn close(a: Pt, b: Pt) -> bool {
    (a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3
}

fn rot(q: Pt, deg: f32) -> Pt {
    let (s, c) = deg.to_radians().sin_cos();
    Pt { x: q.x * c - q.y * s, y: q.x * s + q.y * c }
}

fn anchor(s: &Scene, o: &ObjRef) -> Pt {
    s.anchors.iter().find(|(x, _, _)| x == o).map(|x| x.1).unwrap_or_else(|| panic!("no anchor {o:?}"))
}

// ---------------------------------------------------------------- parse + format

#[test]
fn defaults_are_written_nowhere() {
    for src in ["ngon P 4", "ngon P 5 labels a,b,c,d,e axes rotation", "blob G {e, g}", "cayley D4 <r, s>"] {
        assert_eq!(fmt(src), format!("{src}\n"));
    }
    let g = ngon_of(&fig("ngon P 4"), "P").clone();
    assert_eq!((g.angle, g.show_labels, g.shaded, g.color.clone()), (0.0, true, true, None));
}

#[test]
fn new_ngon_options_round_trip() {
    let f = fig("ngon P 4 angle=45 labels=none");
    let g = ngon_of(&f, "P");
    assert_eq!(g.angle, 45.0);
    assert!(!g.show_labels);
    assert_eq!(g.labels, ["1", "2", "3", "4"], "hidden labels are remembered");
    // (review U14) her option order is kept
    assert_eq!(fmt("ngon P 4 angle=45 labels=none"), "ngon P 4 angle=45 labels=none\n");

    let f = fig("ngon Q 6 plain color=teal labels a,b,,d,e,f");
    let g = ngon_of(&f, "Q");
    assert!(!g.shaded);
    assert_eq!(g.color.as_deref(), Some("teal"));
    assert_eq!(g.labels, ["a", "b", "", "d", "e", "f"]);
    assert_eq!(format(&f), "ngon Q 6 plain color=teal labels a,b,,d,e,f\n");

    // a hidden last label is written `""`, so nothing after it joins the list
    let f = fig("ngon R 4 labels 1,2,3,\"\" axes");
    assert_eq!(ngon_of(&f, "R").labels, ["1", "2", "3", ""]);
    assert!(ngon_of(&f, "R").all_axes);
    assert_eq!(format(&f), "ngon R 4 labels 1,2,3,\"\" axes\n");
    // …and at the end of the line a bare trailing comma means the same
    assert_eq!(ngon_of(&fig("ngon R 4 labels 1,2,3,"), "R").labels, ["1", "2", "3", ""]);
    // a stray trailing comma on a full list isn't a fifth label
    assert_eq!(ngon_of(&fig("ngon R 4 labels a,b,c,d,"), "R").labels, ["a", "b", "c", "d"]);

    // labels kept while hidden; `labels none` / `labels=off` read the same
    assert_eq!(fmt("ngon P 3 labels x,y,z labels=none"), "ngon P 3 labels x,y,z labels=none\n");
    assert!(!ngon_of(&fig("ngon P 3 labels none"), "P").show_labels);
    assert!(!ngon_of(&fig("ngon P 3 labels=off"), "P").show_labels);
    assert!(ngon_of(&fig("ngon P 3 shaded"), "P").shaded);
    // degrees may carry a unit
    assert_eq!(ngon_of(&fig("ngon P 3 angle=22.5°"), "P").angle, 22.5);
    assert_eq!(ngon_of(&fig("ngon P 3 angle=-30deg"), "P").angle, -30.0);
}

#[test]
fn angle_on_blobs_and_cayley_graphs() {
    let f = fig("blob G {e, g} shaded angle=20\ncayley D4 <r, s> angle=-15 legend");
    assert_eq!(blobs(&f)[0].angle, 20.0);
    assert_eq!(cayleys(&f)[0].angle, -15.0);
    assert_eq!(format(&f), "blob G {e, g} shaded angle=20\ncayley D4 <r, s> angle=-15 legend\n");
}

#[test]
fn spelling_survives_the_new_options() {
    let src = "ngon P 4 labels φ,,ψ,ε angle=30 plain";
    let f = fig(src);
    assert_eq!(ngon_of(&f, "P").labels, ["\\varphi", "", "\\psi", "\\varepsilon"]);
    assert_eq!(format(&f), "ngon P 4 labels φ,,ψ,ε angle=30 plain\n");
}

#[test]
fn bad_values_are_warnings() {
    let (f, d) = parse("ngon P 4 angle=x color=pink\nblob G angle=?");
    assert_eq!(warnings(&d).len(), 3, "{d:#?}");
    assert_eq!(ngon_of(&f, "P").angle, 0.0);
    assert_eq!(ngon_of(&f, "P").color, None);
}

// ---------------------------------------------------------------- layout + TikZ

#[test]
fn angle_turns_the_ngon_and_its_labels() {
    let s0 = layout(&fig("ngon P 5 axis 1 rotation"));
    let s1 = layout(&fig("ngon P 5 axis 1 rotation angle=40"));
    let c = anchor(&s0, &ObjRef::NGon("P".into()));
    assert!(close(c, anchor(&s1, &ObjRef::NGon("P".into()))));
    for k in 1..=5 {
        let o = ObjRef::NGonVertex("P".into(), k);
        let v0 = anchor(&s0, &o);
        let v1 = anchor(&s1, &o);
        let want = Pt { x: c.x + rot(Pt { x: v0.x - c.x, y: v0.y - c.y }, 40.0).x, y: c.y + rot(Pt { x: v0.x - c.x, y: v0.y - c.y }, 40.0).y };
        assert!(close(v1, want), "vertex {k}: {v1:?} vs {want:?}");
        // its label sits out along the turned radius
        let at = s1.draws.iter().find_map(|d| match d { Draw::Label { obj, at, .. } if *obj == o => Some(*at), _ => None }).unwrap();
        let (u, w) = (Pt { x: v1.x - c.x, y: v1.y - c.y }, Pt { x: at.x - c.x, y: at.y - c.y });
        let cross = u.x * w.y - u.y * w.x;
        let dot = u.x * w.x + u.y * w.y;
        assert!(dot > 0.0 && cross.abs() / dot < 0.02, "label {k} isn't on its vertex's ray");
    }
    // the axis and the arc turn too: the axis path's direction is 40° more
    let dir = |s: &Scene| {
        s.draws.iter().find_map(|d| match d {
            Draw::Path { obj: ObjRef::NGon(_), start, segs, closed: false, stroke: Some(st), .. } if st.line == Line::Dashed => match segs[0] {
                Seg::Line(e) => Some((e.y - start.y).atan2(e.x - start.x).to_degrees()),
                _ => None,
            },
            _ => None,
        }).unwrap()
    };
    let d = (dir(&s1) - dir(&s0)).rem_euclid(360.0);
    assert!((d - 40.0).abs() < 0.01, "axis turned {d}");
    let arc_label = |s: &Scene| s.draws.iter().find_map(|d| match d { Draw::Label { obj: ObjRef::NGonRotation(_), at, .. } => Some(*at), _ => None }).unwrap();
    let (a0, a1) = (arc_label(&s0), arc_label(&s1));
    let ang = |q: Pt| (q.y - c.y).atan2(q.x - c.x).to_degrees();
    let d = (ang(a1) - ang(a0)).rem_euclid(360.0);
    assert!((d - 40.0).abs() < 6.0, "the r label moved {d}° (want ≈ 40)");
}

#[test]
fn every_vertex_label_is_its_own_drawable() {
    let s = layout(&fig("ngon P 6 labels a,b,c,d,e,f turn=1"));
    for k in 1..=6 {
        let n = s.draws.iter().filter(|d| matches!(d, Draw::Label { obj: ObjRef::NGonVertex(m, j), .. } if m == "P" && *j == k)).count();
        assert_eq!(n, 1, "vertex {k}");
    }
    // which label is at position k, for editing it
    let f = fig("ngon P 6 labels a,b,c,d,e,f turn=1");
    let g = ngon_of(&f, "P");
    let drawn = |k: u32| s.draws.iter().find_map(|d| match d { Draw::Label { obj: ObjRef::NGonVertex(_, j), latex, .. } if *j == k => Some(latex.clone()), _ => None }).unwrap();
    for k in 1..=6 {
        assert_eq!(g.labels[ngon_label_index(g, k)], drawn(k));
    }
    let f = fig("ngon P 4 labels φ,b,c,d flip=1");
    let g = ngon_of(&f, "P");
    let k = (1..=4).find(|&k| ngon_label_index(g, k) == 0).unwrap();
    assert_eq!(as_typed(&f, &ObjRef::NGonVertex("P".into(), k)).as_deref(), Some("φ"));
}

#[test]
fn hidden_labels_are_not_drawn() {
    let s = layout(&fig("ngon P 4 labels=none"));
    assert!(!s.draws.iter().any(|d| matches!(d, Draw::Label { obj: ObjRef::NGonVertex(..), .. })));
    // the vertices are still there to hit and to draw arrows to
    assert_eq!(s.anchors.iter().filter(|(o, _, _)| matches!(o, ObjRef::NGonVertex(..))).count(), 4);
    let s = layout(&fig("ngon P 6 labels 1,2,,4,5,6"));
    let labelled: Vec<u32> = s.draws.iter().filter_map(|d| match d { Draw::Label { obj: ObjRef::NGonVertex(_, k), .. } => Some(*k), _ => None }).collect();
    assert_eq!(labelled, [1, 2, 4, 5, 6]);
    let f = fig("ngon P 4 labels=none");
    let t = ntex_figures::tikz::picture(&f, &layout(&f));
    assert!(!t.contains("{$1$}") && !t.contains("{$2$}"), "{t}");
}

#[test]
fn plain_and_colour() {
    let fill = |src: &str| {
        let s = layout(&fig(src));
        s.draws.iter().find_map(|d| match d { Draw::Path { obj: ObjRef::NGon(_), closed: true, fill, stroke, .. } => Some((fill.clone(), stroke.clone().unwrap().color)), _ => None }).unwrap()
    };
    assert_eq!(fill("ngon P 4"), (Some("blue-tint".into()), "ink".into()));
    assert_eq!(fill("ngon P 4 plain"), (None, "ink".into()));
    assert_eq!(fill("ngon P 4 color=maroon"), (Some("maroon-tint".into()), "maroon".into()));
    assert_eq!(fill("ngon P 4 color=teal plain"), (None, "teal".into()));
    let f = fig("ngon P 4 color=teal plain");
    let t = ntex_figures::tikz::picture(&f, &layout(&f));
    let line = t.lines().find(|l| l.contains("% n-gon P")).unwrap();
    assert!(line.contains("draw=ntexFigTeal") && !line.contains("fill="), "{line}");
}

#[test]
fn turned_blob() {
    let s0 = layout(&fig("blob G \"G\" {e, g, g^2} at (0,0)"));
    let s1 = layout(&fig("blob G \"G\" {e, g, g^2} at (0,0) angle=90"));
    for e in ["e", "g", "g^2"] {
        let o = ObjRef::Element(e.into());
        assert!(close(anchor(&s1, &o), rot(anchor(&s0, &o), 90.0)), "{e}");
    }
    let outline = |s: &Scene| s.draws.iter().find_map(|d| match d { Draw::Path { obj: ObjRef::Blob(_), start, .. } => Some(*start), _ => None }).unwrap();
    assert!(close(outline(&s1), rot(outline(&s0), 90.0)));
    // the label still clears the outline
    let lab = s1.draws.iter().find_map(|d| match d { Draw::Label { obj: ObjRef::Blob(_), at, .. } => Some(*at), _ => None }).unwrap();
    assert!((lab.x * lab.x + lab.y * lab.y).sqrt() > 0.8, "{lab:?}");
    // a child turns with its parent
    let s = layout(&fig("blob G {a, b} angle=30\nblob K in G {c} angle=10"));
    let fr: Vec<&ShapeFrame> = s.shapes.iter().filter(|f| matches!(f.obj, ObjRef::Blob(_))).collect();
    assert_eq!(fr.len(), 2);
    assert_eq!((fr[0].angle, fr[0].inherited), (30.0, 0.0));
    assert_eq!((fr[1].angle, fr[1].inherited), (40.0, 30.0));
}

#[test]
fn turned_cayley_graph() {
    let s0 = layout(&fig("cayley D4 <r, s> radius=2 at (0,0)"));
    let s1 = layout(&fig("cayley D4 <r, s> radius=2 at (0,0) angle=30"));
    let mut n = 0;
    for (o, at, _) in &s0.anchors {
        if let ObjRef::CayleyVertex(..) = o {
            assert!(close(anchor(&s1, o), rot(*at, 30.0)), "{o:?}");
            n += 1;
        }
    }
    assert_eq!(n, 8);
}

#[test]
fn shape_frames_for_handles() {
    let f = fig("scale 1.5\nngon P 4 radius=1.2 angle=15 at (1,1)\nblob G {e} size=3,2 angle=10 at (6,0)\ncayley Z6 <1> radius=1.4 at (0,-5) angle=5");
    let s = layout(&f);
    assert_eq!(s.scale, 1.5);
    assert_eq!(s.shapes.len(), 3);
    let p = &s.shapes[0];
    assert_eq!(p.obj, ObjRef::NGon("P".into()));
    assert!(close(p.centre, Pt { x: 1.5, y: 1.5 }));
    assert!((p.radius - 1.8).abs() < 1e-4 && p.half == (p.radius, p.radius) && p.angle == 15.0);
    for k in 1..=4 {
        let v = anchor(&s, &ObjRef::NGonVertex("P".into(), k));
        let d = ((v.x - p.centre.x).powi(2) + (v.y - p.centre.y).powi(2)).sqrt();
        assert!((d - p.radius).abs() < 1e-3, "vertices lie on the frame's circle");
    }
    let b = &s.shapes[1];
    assert_eq!(b.obj, ObjRef::Blob("G".into()));
    assert!(close(b.centre, Pt { x: 9.0, y: 0.0 }));
    assert!((b.half.0 - 2.25).abs() < 1e-4 && (b.half.1 - 1.5).abs() < 1e-4, "size/2 × scale: {:?}", b.half);
    assert_eq!(b.angle, 10.0);
    let c = &s.shapes[2];
    assert_eq!(c.obj, ObjRef::Cayley("Z6".into()));
    assert!((c.radius - 1.4).abs() < 1e-3, "{}", c.radius);
    // the radius a dragged distance asks for
    let cg = cayleys(&f)[0].clone();
    let r = cayley_radius_for(&cg, 2.1).unwrap();
    assert!((r - 2.1).abs() < 0.011, "{r}");
    let d4 = cayleys(&fig("cayley D4 <r, s>"))[0].clone();
    let r = cayley_radius_for(&d4, 3.0).unwrap();
    let s = layout(&fig(&format!("cayley D4 <r, s> radius={r}")));
    assert!((s.shapes[0].radius - 3.0).abs() < 0.02, "{}", s.shapes[0].radius);
}

// ---------------------------------------------------------------- SetItem

#[test]
fn set_item_ngon_keeps_untouched_spelling() {
    let mut f = fig("ngon P 4 labels φ,ψ,c,d\nP.1 -> P.3");
    let i = index_of(&f, |it| matches!(it, Item::NGon(_)));
    let mut g = ngon_of(&f, "P").clone();
    // the window sends what she sees: her spelling for untouched labels
    g.labels = vec!["φ".into(), "ε".into(), "".into(), "\\delta".into()];
    g.angle = 45.0;
    g.show_labels = true;
    g.shaded = false;
    g.color = Some("violet".into());
    g.radius = Some(1.4);
    let (r, out) = ed(&mut f, Edit::SetItem { index: i, item: Item::NGon(g) });
    assert_eq!(r.unwrap().as_deref(), Some("P"));
    assert_eq!(out, "ngon P 4 labels φ,ε,,\\delta radius=1.4 angle=45 plain color=violet\nP.1 -> P.3\n");
    assert_eq!(ngon_of(&f, "P").labels, ["\\varphi", "\\varepsilon", "", "\\delta"]);
}

#[test]
fn set_item_ngon_fewer_sides_drops_dead_arrows() {
    let mut f = fig("ngon P 6\nP.1 -> P.6\nP.1 -> P.2");
    let mut g = ngon_of(&f, "P").clone();
    g.n = 4;
    g.axes = vec![12];
    let (r, out) = ed(&mut f, Edit::SetItem { index: 0, item: Item::NGon(g) });
    r.unwrap();
    assert_eq!(out, "ngon P 4\nP.1 -> P.2\n");
}

#[test]
fn set_item_node_and_text() {
    let mut f = fig("A \"φ(G)\" -f-> B\ntext \"kernel ↦ e\" at (0,-1)");
    let i = index_of(&f, |it| matches!(it, Item::Node(n) if n.name == "A"));
    let Item::Node(mut n) = f.items[i].clone() else { unreachable!() };
    n.color = Some("teal".into());
    n.label = "φ(G)".into(); // unchanged, as typed
    let (_, out) = ed(&mut f, Edit::SetItem { index: i, item: Item::Node(n.clone()) });
    assert!(out.starts_with("A \"φ(G)\""), "{out}");
    n.label = "ℤ".into();
    let (_, out) = ed(&mut f, Edit::SetItem { index: i, item: Item::Node(n.clone()) });
    assert_eq!(node(&f, "A").label, "\\mathbb{Z}");
    assert!(out.contains("\"ℤ\""), "{out}");
    let t = index_of(&f, |it| matches!(it, Item::Text(_)));
    let (_, out) = ed(&mut f, Edit::SetItem { index: t, item: Item::Text(Text { text: "kernel ↦ e".into(), pos: Pos::Abs(1.0, -1.0), span: Span::default(), color: None, size: None }) });
    assert!(out.ends_with("text \"kernel ↦ e\" at (1,-1)\n"), "{out}");
    // a rename follows into the arrows
    n.name = "X".into();
    let (r, out) = ed(&mut f, Edit::SetItem { index: i, item: Item::Node(n) });
    assert_eq!(r.unwrap().as_deref(), Some("X"));
    assert_eq!(edges(&f), [e("X", "B", Some("f"))]);
    assert!(out.contains("\"ℤ\""), "spelling follows a rename: {out}");
}

#[test]
fn set_item_blob_elements() {
    let mut f = fig("blob G \"G\" {e, g, g^2}\nblob H {h}\ng |-> h\ng^2 |-> h");
    let i = index_of(&f, |it| matches!(it, Item::Blob(b) if b.name == "G"));
    let mut b = blobs(&f)[0].clone();
    b.elements.retain(|x| x.name != "g^2");
    b.elements.push(Element { name: String::new(), label: "k".into(), pos: None });
    b.elements[0].label = "ε".into();
    b.angle = 25.0;
    b.size = Some((2.5, 2.0));
    let (r, out) = ed(&mut f, Edit::SetItem { index: i, item: Item::Blob(b) });
    r.unwrap();
    assert_eq!(out, "blob G \"G\" {e \"ε\", g, k} size=2.5,2 angle=25\nblob H {h}\ng |-> h\n");
    // a name that's taken is refused, and nothing changes
    let mut b = blobs(&f)[0].clone();
    b.elements.push(Element { name: "h".into(), label: "h".into(), pos: None });
    assert!(ed(&mut f, Edit::SetItem { index: i, item: Item::Blob(b) }).0.is_err());
    // a parent cycle is refused
    let mut b = blobs(&f)[0].clone();
    b.parent = Some("G".into());
    assert!(ed(&mut f, Edit::SetItem { index: i, item: Item::Blob(b) }).0.is_err());
}

#[test]
fn set_item_cayley_and_arrow() {
    let mut f = fig("cayley ℤ2 × ℤ4\ncayley <a, b | a^4, b^2, (ab)^2>\nA -φ-> B");
    let i = index_of(&f, |it| matches!(it, Item::Cayley(_)));
    let mut c = cayleys(&f)[0].clone();
    c.angle = 30.0;
    c.legend = true;
    c.group = GroupSpec::Named("ℤ2 × ℤ4".into()); // as she sees it
    let (_, out) = ed(&mut f, Edit::SetItem { index: i, item: Item::Cayley(c.clone()) });
    assert!(out.starts_with("cayley ℤ2 × ℤ4 legend angle=30\n"), "{out}");
    // a new group renames an automatically named graph
    c = cayleys(&f)[0].clone();
    c.group = GroupSpec::Named("D4".into());
    let (r, out) = ed(&mut f, Edit::SetItem { index: i, item: Item::Cayley(c.clone()) });
    assert_eq!(r.unwrap().as_deref(), Some("D4"));
    assert!(out.starts_with("cayley D4 legend angle=30\n"), "{out}");
    c = cayleys(&f)[0].clone();
    c.group = GroupSpec::Named("Q13".into());
    assert!(ed(&mut f, Edit::SetItem { index: i, item: Item::Cayley(c) }).0.is_err());
    let j = index_of(&f, |it| matches!(it, Item::Cayley(c) if c.name == "C"));
    let mut c = cayleys(&f)[1].clone();
    c.group = GroupSpec::Presentation(vec!["a".into(), "b".into()], vec!["a^3".into(), "b^2".into(), "(ab)^2".into()]);
    let (_, out) = ed(&mut f, Edit::SetItem { index: j, item: Item::Cayley(c) });
    assert!(out.contains("cayley <a, b | a^3, b^2, (ab)^2>"), "{out}");
    // an arrow: unchanged label keeps φ; the kind must match
    let k = index_of(&f, |it| matches!(it, Item::Arrow(_)));
    let Item::Arrow(mut a) = f.items[k].clone() else { unreachable!() };
    a.label = Some("φ".into());
    a.head = Head::TwoHeads;
    let (_, out) = ed(&mut f, Edit::SetItem { index: k, item: Item::Arrow(a) });
    assert!(out.ends_with("A -φ->> B\n"), "{out}");
    assert!(ed(&mut f, Edit::SetItem { index: k, item: Item::Text(Text { text: "x".into(), pos: Pos::Abs(0.0, 0.0), span: Span::default(), color: None, size: None }) }).0.is_err());
    assert!(ed(&mut f, Edit::SetItem { index: 99, item: Item::Text(Text { text: "x".into(), pos: Pos::Abs(0.0, 0.0), span: Span::default(), color: None, size: None }) }).0.is_err());
}

#[test]
fn set_item_on_a_grid_node_keeps_the_block_unless_it_moves() {
    // (FIG-INTERACT, 2026-10-12, by the coordinator's request) a relabel
    // keeps her ASCII art (the label goes after `end`); a move converts it
    let mut f = fig("grid\n  A --f--> B\n  |g       |h\n  v        v\n  C --k--> D\nend");
    let i = index_of(&f, |it| matches!(it, Item::Node(n) if n.name == "B"));
    let Item::Node(mut n) = f.items[i].clone() else { unreachable!() };
    n.label = "G/K".into();
    let (_, out) = ed(&mut f, Edit::SetItem { index: i, item: Item::Node(n.clone()) });
    assert!(!out.contains("% grid converted to statements") && out.contains("A --f--> B"), "{out}");
    assert_eq!(node(&f, "B").label, "G/K");
    n.pos = Some(Pos::Cell(3.0, 0.0));
    let (_, out) = ed(&mut f, Edit::SetItem { index: i, item: Item::Node(n) });
    assert!(out.contains("% grid converted to statements"), "{out}");
}

// ---------------------------------------------------------------- MoveObj

#[test]
fn move_obj_moves_everything_that_has_a_position() {
    let mut f = fig("grid\n  A --> B\nend\nblob G {e, g}\nngon P 4\ncayley D4\ntext \"t\" at (0,0)");
    let t = index_of(&f, |it| matches!(it, Item::Text(_)));
    let moves = [
        ObjRef::Node("A".into()),
        ObjRef::Blob("G".into()),
        ObjRef::Element("g".into()),
        ObjRef::NGon("P".into()),
        ObjRef::Cayley("D4".into()),
        ObjRef::Text(t),
    ];
    for (k, o) in moves.iter().enumerate() {
        let pos = if k == 0 { Pos::Cell(0.0, 2.0) } else { Pos::Abs(k as f32, -1.5) };
        let (r, _) = ed(&mut f, Edit::MoveObj { obj: o.clone(), pos });
        r.unwrap_or_else(|e| panic!("{o:?}: {e}"));
    }
    let out = format(&f);
    assert!(out.contains("% grid converted to statements"), "{out}");
    assert_eq!(cell(&f, "A"), (0.0, 2.0));
    assert_eq!(blobs(&f)[0].pos, Some(Pos::Abs(1.0, -1.5)));
    assert_eq!(blobs(&f)[0].elements[1].pos, Some(Pos::Abs(2.0, -1.5)));
    assert_eq!(ngon_of(&f, "P").pos, Some(Pos::Abs(3.0, -1.5)));
    assert_eq!(cayleys(&f)[0].pos, Some(Pos::Abs(4.0, -1.5)));
    assert!(out.contains("text \"t\" at (5,-1.5)"), "{out}");
    for bad in [ObjRef::NGonVertex("P".into(), 1), ObjRef::Arrow(0), ObjRef::Node("Nope".into())] {
        assert!(ed(&mut f, Edit::MoveObj { obj: bad, pos: Pos::Abs(0.0, 0.0) }).0.is_err());
    }
    assert!(ed(&mut f, Edit::MoveObj { obj: ObjRef::NGon("P".into()), pos: Pos::Abs(f32::NAN, 0.0) }).0.is_err());
}

// ---------------------------------------------------------------- Duplicate

#[test]
fn duplicate_ngon_blob_cayley_node_text() {
    let mut f = fig("ngon P 4 labels φ,b,c,d angle=45 at (0,0)\nngon Q 3");
    let (r, out) = ed(&mut f, Edit::Duplicate { obj: ObjRef::NGon("P".into()) });
    assert_eq!(r.unwrap().as_deref(), Some("P2"));
    assert!(out.ends_with("ngon P2 4 labels φ,b,c,d angle=45 at (1,-1)\n"), "{out}");
    let (r, _) = ed(&mut f, Edit::Duplicate { obj: ObjRef::NGonVertex("P".into(), 2) });
    assert_eq!(r.unwrap().as_deref(), Some("P3"));
    let (r, out) = ed(&mut f, Edit::Duplicate { obj: ObjRef::NGon("Q".into()) });
    assert_eq!(r.unwrap().as_deref(), Some("Q2"));
    assert!(out.ends_with("ngon Q2 3\n"), "an unplaced shape joins the row: {out}");

    let mut f = fig("blob G \"G\" {e, g \"φ\", g^2} shaded\nblob H {h}");
    let (r, out) = ed(&mut f, Edit::Duplicate { obj: ObjRef::Blob("G".into()) });
    assert_eq!(r.unwrap().as_deref(), Some("G'"));
    assert!(out.ends_with("blob G' {e', g' \"φ\", g^2'} shaded\n"), "{out}");
    let (r, _) = ed(&mut f, Edit::Duplicate { obj: ObjRef::Blob("G".into()) });
    assert_eq!(r.unwrap().as_deref(), Some("G''"));
    let (r, out) = ed(&mut f, Edit::Duplicate { obj: ObjRef::Element("h".into()) });
    assert_eq!(r.unwrap().as_deref(), Some("h'"));
    assert!(out.contains("blob H {h, h'}"), "{out}");

    let mut f = fig("cayley ℤ2 × ℤ4 <a, b> at (0,0)");
    let (r, out) = ed(&mut f, Edit::Duplicate { obj: ObjRef::CayleyVertex("Z2xZ4".into(), "a".into()) });
    assert_eq!(r.unwrap().as_deref(), Some("Z2xZ4'"));
    assert!(out.ends_with("cayley ℤ2 × ℤ4 <a, b> name=Z2xZ4' at (1,-1)\n"), "{out}");

    let mut f = fig("A \"φ\" at 0,0\nB at 1,0\nA -> B\ntext \"t\" at (0,-2)");
    let (r, out) = ed(&mut f, Edit::Duplicate { obj: ObjRef::Node("A".into()) });
    assert_eq!(r.unwrap().as_deref(), Some("C"));
    assert!(out.contains("C \"φ\" at 2,0"), "next free cell to the right: {out}");
    let t = index_of(&f, |it| matches!(it, Item::Text(_)));
    let (_, out) = ed(&mut f, Edit::Duplicate { obj: ObjRef::Text(t) });
    assert!(out.ends_with("text \"t\" at (1,-2)\n"), "{out}");
    assert!(ed(&mut f, Edit::Duplicate { obj: ObjRef::Blob("nope".into()) }).0.is_err());
}

// ---------------------------------------------------------------- property

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545F4914F6CDD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { (self.next() % n as u64) as usize }
    }
}

const STARTS: &[&str] = &[
    "grid\n  A --f--> B\n  |g       |h\n  v        v\n  C --k--> D\nend\nD -> A bend-left",
    "blob G \"G\" {e, g, g^2, g^3}\nblob K \"\\ker\\varphi\" in G shaded {e, g^2}\nblob H \"H\" {e', h} angle=15\nG -φ-> H\ng |-> h ; e |-> e'",
    "ngon P 4 labels φ,,c,d angle=30\nngon Q 6 axis 1 axis 2.5 rotation=2 plain color=teal\nP.1 -> Q.2\ncayley D4 <r, s> angle=10",
    "cayley <a, b | a^4, b^2, (ab)^2>\ncayley ℤ2 × ℤ4\ntext \"kernel ↦ e\" at (0,-3)",
    "A -f-> B -g-> C\nB <-h- D % back\nA at 0,0",
    "",
];

fn every_obj(f: &Figure) -> Vec<ObjRef> {
    let mut v = vec![];
    for (i, it) in f.items.iter().enumerate() {
        match it {
            Item::Node(n) => v.push(ObjRef::Node(n.name.clone())),
            Item::Arrow(_) => v.push(ObjRef::Arrow(i)),
            Item::Blob(b) => {
                v.push(ObjRef::Blob(b.name.clone()));
                v.extend(b.elements.iter().map(|e| ObjRef::Element(e.name.clone())));
            }
            Item::NGon(g) => {
                v.push(ObjRef::NGon(g.name.clone()));
                v.push(ObjRef::NGonVertex(g.name.clone(), 1));
            }
            Item::Cayley(c) => v.push(ObjRef::Cayley(c.name.clone())),
            Item::Text(_) => v.push(ObjRef::Text(i)),
            _ => {}
        }
    }
    v
}

const TYPED: &[&str] = &["φ", "x", "", "\\psi", "ℤ", "g ∘ f", "a\"b", "q,r"];

fn tweak(r: &mut Rng, it: &Item) -> Item {
    let mut it = it.clone();
    let typed = |r: &mut Rng| TYPED[r.below(TYPED.len())].to_string();
    match &mut it {
        Item::Node(n) => {
            n.label = typed(r);
            n.color = [None, Some("blue".to_string()), Some("pink".to_string())][r.below(3)].clone();
            if r.below(4) == 0 {
                n.name = ["Y", "B", "Z'", "P.1"][r.below(4)].into();
            }
        }
        Item::Arrow(a) => {
            a.label = Some(typed(r)).filter(|_| r.below(3) > 0);
            a.bend = [0.0, 30.0, -45.0][r.below(3)];
            a.side = if r.below(2) == 0 { LabelSide::Swap } else { LabelSide::Auto };
        }
        Item::Blob(b) => {
            b.angle = [0.0, 20.0, -90.0, 7.5][r.below(4)];
            b.shaded = r.below(2) == 0;
            b.label = typed(r);
            if !b.elements.is_empty() && r.below(3) == 0 {
                b.elements.remove(r.below(b.elements.len()));
            }
            if r.below(3) == 0 {
                b.elements.push(Element { name: ["", "w", "e"][r.below(3)].into(), label: typed(r), pos: None });
            }
            for e in b.elements.iter_mut() {
                if r.below(3) == 0 {
                    e.label = typed(r);
                }
            }
            b.size = [None, Some((2.0, 1.5))][r.below(2)];
        }
        Item::NGon(g) => {
            g.n = [3, 4, 5, 6, 8, 2][r.below(6)];
            g.angle = [0.0, 45.0, -15.0, 360.0][r.below(4)];
            g.show_labels = r.below(3) > 0;
            g.shaded = r.below(2) == 0;
            g.color = [None, Some("maroon".to_string()), Some("teal".to_string())][r.below(3)].clone();
            for l in g.labels.iter_mut() {
                if r.below(3) == 0 {
                    *l = typed(r);
                }
            }
            g.axes = vec![[2, 3, 4, 9, 40][r.below(5)]];
            g.flip = [None, Some(2), Some(5)][r.below(3)];
        }
        Item::Cayley(c) => {
            c.angle = [0.0, 30.0, 90.0][r.below(3)];
            c.labels = r.below(2) == 0;
            if let GroupSpec::Named(g) = &mut c.group {
                *g = ["D4", "ℤ6", "Z2 x Z2", "S3", "Q8"][r.below(5)].into();
            }
        }
        Item::Text(t) => t.text = ["t", "x ↦ y", ""][r.below(3)].into(),
        _ => {}
    }
    it
}

fn random_edit(r: &mut Rng, f: &Figure) -> Edit {
    let objs = every_obj(f);
    let pick = |r: &mut Rng| objs.get(r.below(objs.len())).cloned().unwrap_or(ObjRef::Node("A".into()));
    let pos = |r: &mut Rng| if r.below(2) == 0 { Pos::Cell(r.below(4) as f32, r.below(3) as f32) } else { Pos::Abs(r.below(5) as f32 * 0.5, -(r.below(3) as f32)) };
    match r.below(10) {
        0..=3 => {
            let i = r.below(f.items.len().max(1));
            match f.items.get(i) {
                Some(it) => Edit::SetItem { index: i, item: tweak(r, it) },
                None => Edit::AddNGon { n: 4, pos: pos(r), radius: 1.0 },
            }
        }
        4 | 5 => Edit::MoveObj { obj: pick(r), pos: pos(r) },
        6 | 7 => Edit::Duplicate { obj: pick(r) },
        8 => Edit::Delete { obj: pick(r) },
        _ => match r.below(3) {
            0 => Edit::AddNode { label: "G".into(), pos: pos(r) },
            1 => Edit::AddNGon { n: 3 + r.below(4) as u32, pos: pos(r), radius: 1.0 },
            _ => Edit::AddText { text: "u".into(), pos: pos(r) },
        },
    }
}

#[test]
fn property_random_interaction_edits_keep_the_round_trip() {
    let mut r = Rng(0x9E3779B97F4A7C15);
    let (mut applied, mut kinds) = (0, [0usize; 3]);
    for round in 0..240 {
        let mut f = fig(STARTS[round % STARTS.len()]);
        for _ in 0..10 {
            let e = random_edit(&mut r, &f);
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut g = f.clone();
                let r = apply(&mut g, &e);
                (g, r)
            }));
            let (g, res) = res.unwrap_or_else(|_| panic!("edit panicked: {e:?} on\n{}", format(&f)));
            if res.is_ok() {
                applied += 1;
                match e {
                    Edit::SetItem { .. } => kinds[0] += 1,
                    Edit::MoveObj { .. } => kinds[1] += 1,
                    Edit::Duplicate { .. } => kinds[2] += 1,
                    _ => {}
                }
                let out = format(&g);
                let (h, _) = parse(&out);
                assert_eq!(without_spans(&h), without_spans(&g), "after {e:?}\n--- before:\n{}\n--- after:\n{out}", format(&f));
                assert_eq!(format(&h), out, "not idempotent after {e:?}");
                // and it still lays out
                let s = std::panic::catch_unwind(|| layout(&g));
                assert!(s.is_ok(), "layout panicked after {e:?}:\n{out}");
                f = g;
            } else {
                assert_eq!(g, f, "a refused edit changed the figure: {e:?}");
            }
        }
    }
    assert!(applied > 1200 && kinds.iter().all(|&k| k > 150), "applied {applied}, by kind {kinds:?}");
}
