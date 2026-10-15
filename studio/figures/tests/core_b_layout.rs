//! layout → Scene, and tidy (FIG-CORE-B). Figures are built by hand.
mod core_b_common;
use core_b_common::*;
use ntex_figures::layout::*;
use ntex_figures::model::*;

fn node_at(s: &Scene, name: &str) -> Pt {
    s.anchors.iter().find(|(o, _, _)| *o == ObjRef::Node(name.into())).map(|(_, p, _)| *p).unwrap_or_else(|| panic!("no node {name}"))
}
fn cell_of(s: &Scene, name: &str) -> (f32, f32) {
    let p = node_at(s, name);
    (p.x / s.cell.0, -p.y / s.cell.1)
}
fn anchor_rect(s: &Scene, o: &ObjRef) -> Rect {
    s.anchors.iter().find(|(x, _, _)| x == o).map(|(_, _, r)| r.clone()).unwrap()
}
fn paths<'a>(s: &'a Scene, f: impl Fn(&ObjRef) -> bool + 'a) -> impl Iterator<Item = &'a Draw> + 'a {
    s.draws.iter().filter(move |d| matches!(d, Draw::Path { obj, .. } if f(obj)))
}
fn near(a: f32, b: f32, e: f32) -> bool {
    (a - b).abs() <= e
}
fn on_border(p: Pt, r: &Rect) -> bool {
    let e = 1e-3;
    let inside = p.x >= r.min.x - e && p.x <= r.max.x + e && p.y >= r.min.y - e && p.y <= r.max.y + e;
    inside && (near(p.x, r.min.x, e) || near(p.x, r.max.x, e) || near(p.y, r.min.y, e) || near(p.y, r.max.y, e))
}
fn end_of(segs: &[Seg]) -> Pt {
    match segs.last().unwrap() {
        Seg::Line(e) | Seg::Cubic(_, _, e) => *e,
    }
}

#[test]
fn a_four_cycle_becomes_a_square() {
    let f = fig(vec![a("A", "B", Some("f")), a("A", "C", Some("g")), a("B", "D", Some("h")), a("C", "D", Some("k"))]);
    let s = layout(&f);
    assert_eq!(cell_of(&s, "A"), (0.0, 0.0));
    assert_eq!(cell_of(&s, "B"), (1.0, 0.0));
    assert_eq!(cell_of(&s, "C"), (0.0, 1.0));
    assert_eq!(cell_of(&s, "D"), (1.0, 1.0));
    assert_eq!(s.cell, DEFAULT_SPACING);
}

#[test]
fn a_three_cycle_becomes_a_triangle() {
    let f = fig(vec![a("G", "Q", None), a("Q", "H", None), a("G", "H", None)]);
    let s = layout(&f);
    assert_eq!(cell_of(&s, "G"), (0.0, 0.0));
    assert_eq!(cell_of(&s, "Q"), (1.0, 0.0));
    assert_eq!(cell_of(&s, "H"), (1.0, 1.0));
}

#[test]
fn chains_go_left_to_right_and_fans_branch() {
    let f = fig(vec![a("A", "B", None), a("B", "C", None), a("C", "D", None)]);
    let s = layout(&f);
    for (i, n) in ["A", "B", "C", "D"].iter().enumerate() {
        assert_eq!(cell_of(&s, n), (i as f32, 0.0));
    }
    let f = fig(vec![a("A", "B", None), a("A", "C", None), a("A", "D", None)]);
    let s = layout(&f);
    let (b, c, d) = (cell_of(&s, "B"), cell_of(&s, "C"), cell_of(&s, "D"));
    assert!(b.0 == 1.0 && c.0 == 1.0 && d.0 == 1.0, "children in the next column");
    let a0 = cell_of(&s, "A");
    let mut rows = vec![b.1 - a0.1, c.1 - a0.1, d.1 - a0.1];
    rows.sort_by(|x, y| x.partial_cmp(y).unwrap());
    assert_eq!(rows, vec![-1.0, 0.0, 1.0], "a fan of three is centred on its source");
    assert_eq!(a0.0, 0.0);
    // a fan of two: the first child stays on the source's row
    let s2 = layout(&fig(vec![a("A", "B", None), a("A", "C", None)]));
    assert_eq!(cell_of(&s2, "B"), (1.0, cell_of(&s2, "A").1));
    assert_eq!(cell_of(&s2, "C"), (1.0, cell_of(&s2, "A").1 + 1.0));
    // every auto cell is an integer cell
    for (o, p, _) in &s.anchors {
        if let ObjRef::Node(_) = o {
            assert!(near((p.x / s.cell.0).fract(), 0.0, 1e-4) && near((p.y / s.cell.1).fract(), 0.0, 1e-4));
        }
    }
}

#[test]
fn unplaced_nodes_go_to_free_cells_next_to_placed_ones() {
    let f = fig(vec![node("A", "A", Some(Pos::Cell(2.0, 1.0))), a("A", "B", None), a("C", "A", None), node("X", "X", Some(Pos::Cell(3.0, 1.0)))]);
    let s = layout(&f);
    assert_eq!(cell_of(&s, "A"), (2.0, 1.0));
    let b = cell_of(&s, "B");
    assert_ne!(b, (3.0, 1.0), "taken by X");
    assert!((b.0 - 2.0).abs() <= 1.0 && (b.1 - 1.0).abs() <= 1.0, "B next to A: {b:?}");
    assert_eq!(cell_of(&s, "C"), (1.0, 1.0), "a source goes to the left");
}

#[test]
fn absolute_positions_spacing_and_scale() {
    let mut f = fig(vec![node("A", "A", Some(Pos::Abs(3.2, -1.0))), node("B", "B", Some(Pos::Cell(1.0, 1.0)))]);
    f.globals.spacing = (2.8, 2.0);
    f.globals.scale = 1.5;
    let s = layout(&f);
    let a = node_at(&s, "A");
    assert!(near(a.x, 4.8, 1e-4) && near(a.y, -1.5, 1e-4));
    let b = node_at(&s, "B");
    assert!(near(b.x, 4.2, 1e-4) && near(b.y, -3.0, 1e-4));
    assert_eq!(s.cell, (4.2, 3.0));
}

#[test]
fn label_widths_are_close_to_charter() {
    // measured with XCharter + newtxmath at 10 pt (\wd of $…$)
    for (latex, w) in [("A", 6.38), ("G/K", 17.77), ("\\ker\\varphi", 23.12), ("X\\times_Z Y", 27.87), ("\\mathbb{Z}/n\\mathbb{Z}", 24.36), ("g\\circ f", 19.75), ("r^2s", 12.23), ("H'", 10.78), ("Z_n", 9.83), ("\\varphi", 7.55)] {
        let (est, _, _) = label_metrics(latex, true);
        assert!((est - w).abs() / w < 0.2, "{latex}: estimated {est:.2}pt, real {w}pt");
    }
    let small = node_box("A", 10.0);
    let big = node_box("A \\times B \\times C", 10.0);
    assert!(big.max.x - big.min.x > 2.0 * (small.max.x - small.min.x));
}

#[test]
fn arrows_are_shortened_to_the_node_boxes() {
    let f = fig(vec![a("A", "B", Some("f")), a("A", "C", Some("g")), a("B", "D", Some("h")), a("C", "D", Some("k"))]);
    let s = layout(&f);
    let names = [("A", "B"), ("A", "C"), ("B", "D"), ("C", "D")];
    for (i, (x, y)) in names.iter().enumerate() {
        let Some(Draw::Path { start, segs, end_tip, .. }) = paths(&s, |o| *o == ObjRef::Arrow(i)).next() else { panic!() };
        assert!(on_border(*start, &anchor_rect(&s, &ObjRef::Node(x.to_string()))), "{x}->{y} starts on {x}'s box");
        assert!(on_border(end_of(segs), &anchor_rect(&s, &ObjRef::Node(y.to_string()))), "{x}->{y} ends on {y}'s box");
        assert_eq!(*end_tip, Some(Head::To));
    }
    // labels: f above the top arrow, g left of the left arrow (tikz-cd's auto side)
    let lab = |i: usize| s.draws.iter().find_map(|d| if let Draw::Label { obj: ObjRef::Arrow(j), at, .. } = d { (*j == i).then_some(*at) } else { None }).unwrap();
    let (a_, b_) = (node_at(&s, "A"), node_at(&s, "B"));
    assert!(lab(0).y > a_.y && lab(0).x > a_.x && lab(0).x < b_.x);
    assert!(lab(1).x > a_.x, "a downward arrow's auto label is on its left = east");
}

#[test]
fn swap_and_near_end_move_the_label() {
    let f = fig(vec![node("A", "A", Some(Pos::Cell(0.0, 0.0))), node("B", "B", Some(Pos::Cell(1.0, 0.0))), with(arr("A", "B", Some("f")), |a| {
        a.side = LabelSide::Swap;
        a.label_pos = LabelPos::NearEnd
    })]);
    let s = layout(&f);
    let at = s.draws.iter().find_map(|d| if let Draw::Label { obj: ObjRef::Arrow(_), at, .. } = d { Some(*at) } else { None }).unwrap();
    assert!(at.y < 0.0, "swap puts it below");
    assert!(at.x > DEFAULT_SPACING.0 * 0.6, "near end: past the middle");
}

#[test]
fn bends_and_loops() {
    let f = fig(vec![
        node("A", "A", Some(Pos::Cell(0.0, 0.0))),
        node("B", "B", Some(Pos::Cell(1.0, 0.0))),
        with(arr("A", "B", None), |a| a.bend = 30.0),
        with(arr("A", "B", None), |a| a.bend = -30.0),
        with(arr("A", "A", Some("\\mathrm{id}")), |a| a.is_loop = true),
    ]);
    let s = layout(&f);
    let ctrl = |i: usize| -> (Pt, Pt, Pt) {
        let Some(Draw::Path { start, segs, .. }) = paths(&s, |o| *o == ObjRef::Arrow(i)).next() else { panic!() };
        match segs[0] {
            Seg::Cubic(c1, _, e) => (*start, c1, e),
            _ => panic!("a bend is a curve"),
        }
    };
    assert!(ctrl(2).1.y > 0.3, "bend left (+) goes up for a rightward arrow");
    assert!(ctrl(3).1.y < -0.3, "bend right goes down");
    let (s0, c1, e) = ctrl(4);
    let r = anchor_rect(&s, &ObjRef::Node("A".into()));
    assert!(on_border(s0, &r) && on_border(e, &r), "a loop starts and ends on its node");
    assert!(c1.y > r.max.y, "and rises above it");
}

fn blob_poly(s: &Scene, name: &str) -> Vec<Pt> {
    let Some(Draw::Path { start, segs, closed, .. }) = paths(s, |o| *o == ObjRef::Blob(name.into())).next() else { panic!("no blob {name}") };
    assert!(closed);
    flatten(*start, segs)
}

fn kernel_fig() -> Figure {
    fig(vec![
        Item::Blob(blob("G", "G", &["g", "gk"], None, false)),
        Item::Blob(blob("K", "\\ker\\varphi", &["e", "k"], Some("G"), true)),
        Item::Blob(blob("H", "H", &["e'", "h"], None, false)),
        a("G", "H", Some("\\varphi")),
        maps("e", "e'"),
        maps("g", "h"),
    ])
}

#[test]
fn blobs_nest_and_hold_their_points() {
    let s = layout(&kernel_fig());
    let (g, k, h) = (blob_poly(&s, "G"), blob_poly(&s, "K"), blob_poly(&s, "H"));
    for q in &k {
        assert!(point_in_poly(*q, &g), "K inside G");
    }
    let dot = |n: &str| s.draws.iter().find_map(|d| if let Draw::Dot { obj: ObjRef::Element(x), at, .. } = d { (x == n).then_some(*at) } else { None }).unwrap();
    for e in ["e", "k"] {
        assert!(point_in_poly(dot(e), &k), "{e} in K");
    }
    for e in ["g", "gk"] {
        assert!(point_in_poly(dot(e), &g) && !point_in_poly(dot(e), &k), "{e} in G, outside K");
    }
    for e in ["e'", "h"] {
        assert!(point_in_poly(dot(e), &h));
    }
    // top-level blobs in a row, left to right
    let gx = g.iter().map(|p| p.x).fold(f32::MIN, f32::max);
    let hx = h.iter().map(|p| p.x).fold(f32::MAX, f32::min);
    assert!(gx < hx);
    // the shaded kernel is filled with a tint
    let Some(Draw::Path { fill, .. }) = paths(&s, |o| *o == ObjRef::Blob("K".into())).next() else { panic!() };
    assert!(fill.as_deref().is_some_and(|f| f.ends_with("-tint")));
    // blob to blob: an arc over the top
    let Some(Draw::Path { start, segs, .. }) = paths(&s, |o| *o == ObjRef::Arrow(3)).next() else { panic!() };
    let top = g.iter().chain(h.iter()).map(|p| p.y).fold(f32::MIN, f32::max);
    assert!(flatten(*start, segs).iter().any(|p| p.y > top), "the arc rises above the blobs");
    // element arrows run point to point, just off the dots
    let Some(Draw::Path { start, segs, start_tip, .. }) = paths(&s, |o| *o == ObjRef::Arrow(4)).next() else { panic!() };
    let d0 = ((start.x - dot("e").x).powi(2) + (start.y - dot("e").y).powi(2)).sqrt();
    let d1 = {
        let e = end_of(segs);
        ((e.x - dot("e'").x).powi(2) + (e.y - dot("e'").y).powi(2)).sqrt()
    };
    assert!(d0 > 0.03 && d0 < 0.15 && d1 > 0.03 && d1 < 0.15);
    assert_eq!(*start_tip, Some(Head::MapsTo));
}

#[test]
fn blob_shapes_are_deterministic_and_seeded_by_name() {
    let s1 = layout(&kernel_fig());
    let s2 = layout(&kernel_fig());
    assert_eq!(s1, s2, "same figure, same scene");
    let one = |name: &str| {
        let f = fig(vec![Item::Blob(blob(name, "X", &["x"], None, false))]);
        let s = layout(&f);
        blob_poly(&s, name)
    };
    let (a1, b1) = (one("A"), one("B"));
    assert_ne!(a1, b1, "different names, different wobble");
    // shapes follow their option
    let shaped = |shape: BlobShape| {
        let f = fig(vec![Item::Blob(Blob { shape, ..blob("S", "S", &["x", "y"], None, false) })]);
        let s = layout(&f);
        let r = anchor_rect(&s, &ObjRef::Blob("S".into()));
        (r.max.x - r.min.x) / (r.max.y - r.min.y)
    };
    assert!(shaped(BlobShape::Wide) > 1.25);
    assert!(shaped(BlobShape::Tall) < 0.8);
    let round = shaped(BlobShape::Round);
    assert!(round > 0.75 && round < 1.25);
    // an explicit size wins
    let f = fig(vec![Item::Blob(Blob { size: Some((4.0, 2.0)), ..blob("S", "S", &[], None, false) })]);
    let s = layout(&f);
    let r = anchor_rect(&s, &ObjRef::Blob("S".into()));
    assert!(near(r.max.x - r.min.x, 4.0, 0.5) && near(r.max.y - r.min.y, 2.0, 0.35));
}

#[test]
fn ngons_turn_flip_axes_rotation() {
    let g = NGon { all_axes: true, rotation: 1, ..ngon("P", 4) };
    let s = layout(&fig(vec![Item::NGon(g.clone())]));
    // a square sits on a flat bottom edge: vertex 1 top-left, counter-clockwise
    let v = |k: u32| s.anchors.iter().find(|(o, _, _)| *o == ObjRef::NGonVertex("P".into(), k)).map(|(_, p, _)| *p).unwrap();
    assert!(v(1).y > v(2).y && v(1).x < 0.0 && near(v(1).x, v(2).x, 1e-4), "vertex 1 top-left, 2 below it");
    assert!(near(v(2).y, v(3).y, 1e-4) && v(3).x > 0.0, "a flat bottom edge 2–3");
    assert!(near(v(1).y, v(4).y, 1e-4) && v(4).x > 0.0, "a flat top edge 4–1");
    // odd n: vertex 1 on top
    let s5 = layout(&fig(vec![Item::NGon(ngon("Q", 5))]));
    let v5 = s5.anchors.iter().find(|(o, _, _)| *o == ObjRef::NGonVertex("Q".into(), 1)).map(|(_, p, _)| *p).unwrap();
    assert!(near(v5.x, 0.0, 1e-4) && v5.y > 0.0);
    let axes = paths(&s, |o| *o == ObjRef::NGon("P".into())).filter(|d| matches!(d, Draw::Path { stroke: Some(st), .. } if st.line == Line::Dashed)).count();
    assert_eq!(axes, 4, "a square has four reflection axes");
    let arcs = paths(&s, |o| *o == ObjRef::NGonRotation("P".into())).filter(|d| matches!(d, Draw::Path { end_tip: Some(_), .. })).count();
    assert_eq!(arcs, 1, "one rotation arrow");
    assert!(s.draws.iter().any(|d| matches!(d, Draw::Label { obj: ObjRef::NGonRotation(_), latex, .. } if latex == "r")));
    // turn: labels move counter-clockwise; flip across the axis through 2
    assert_eq!(ngon_labels_at(&NGon { turn: 1, ..ngon("P", 4) }), vec!["4", "1", "2", "3"]);
    assert_eq!(ngon_labels_at(&NGon { flip: Some(4), ..ngon("P", 4) }), vec!["3", "2", "1", "4"]);
    assert_eq!(ngon_labels_at(&NGon { flip: Some(3), ..ngon("P", 4) }), vec!["2", "1", "4", "3"], "axis through edge 1's midpoint");
    let p5 = layout(&fig(vec![Item::NGon(NGon { all_axes: true, rotation: 2, ..ngon("P", 5) })]));
    let axes5 = paths(&p5, |o| *o == ObjRef::NGon("P".into())).filter(|d| matches!(d, Draw::Path { stroke: Some(st), .. } if st.line == Line::Dashed)).count();
    assert_eq!(axes5, 5);
    assert!(p5.draws.iter().any(|d| matches!(d, Draw::Label { latex, .. } if latex == "r^{2}")));
    // several in a row, left to right
    let s3 = layout(&fig(vec![Item::NGon(ngon("P", 4)), Item::NGon(ngon("Q", 4)), Item::NGon(ngon("R", 4))]));
    let c = |n: &str| s3.anchors.iter().find(|(o, _, _)| *o == ObjRef::NGon(n.into())).map(|(_, p, _)| p.x).unwrap();
    assert!(c("P") < c("Q") && c("Q") < c("R"));
}

#[test]
fn cayley_graphs_colours_involutions_legend() {
    let s = layout(&fig(vec![cayley("C", GroupSpec::Named("D4".into()), &["r", "s"], true)]));
    let verts = s.anchors.iter().filter(|(o, _, _)| matches!(o, ObjRef::CayleyVertex(..))).count();
    assert_eq!(verts, 8);
    let edges: Vec<&Draw> = paths(&s, |o| matches!(o, ObjRef::CayleyEdge(..))).collect();
    let r_edges = edges.iter().filter(|d| matches!(d, Draw::Path { stroke: Some(st), .. } if st.color == "blue")).count();
    let s_edges: Vec<&&Draw> = edges.iter().filter(|d| matches!(d, Draw::Path { stroke: Some(st), .. } if st.color == "maroon")).collect();
    assert_eq!(r_edges, 8, "r: one directed edge per element");
    assert_eq!(s_edges.len(), 4, "s is an involution: each edge once");
    assert!(s_edges.iter().all(|d| matches!(d, Draw::Path { end_tip: None, start_tip: None, .. })), "and with no head");
    assert!(s.draws.iter().any(|d| matches!(d, Draw::Label { obj: ObjRef::Cayley(_), latex, .. } if latex == "r")), "a legend");
    assert!(s.draws.iter().any(|d| matches!(d, Draw::Label { latex, .. } if latex == "r^2")), "shortlex words with powers");
    // prism: e and s are radially aligned (same angle, different radius)
    let at = |w: &str| s.anchors.iter().find(|(o, _, _)| *o == ObjRef::CayleyVertex("C".into(), w.into())).map(|(_, p, _)| *p).unwrap();
    let c = s.anchors.iter().find(|(o, _, _)| *o == ObjRef::Cayley("C".into())).map(|(_, p, _)| *p).unwrap();
    let (e, sv) = (at("e"), at("s"));
    let ang = |p: Pt| (p.y - c.y).atan2(p.x - c.x);
    assert!(near(ang(e), ang(sv), 1e-3));
    assert!(((e.x - c.x).powi(2) + (e.y - c.y).powi(2)).sqrt() > ((sv.x - c.x).powi(2) + (sv.y - c.y).powi(2)).sqrt());
    // labels=none: dots
    let s = layout(&fig(vec![Item::Cayley(Cayley { labels: false, ..match cayley("C", GroupSpec::Named("Z6".into()), &["1"], false) {
        Item::Cayley(c) => c,
        _ => unreachable!(),
    } })]));
    assert_eq!(s.draws.iter().filter(|d| matches!(d, Draw::Dot { obj: ObjRef::CayleyVertex(..), .. })).count(), 6);
    // a group that can't be drawn (infinite) is a diagnostic only: nothing is
    // typeset into the figure (FIG-POLISH)
    let f = fig(vec![cayley("C", GroupSpec::Presentation(vec!["a".into(), "b".into()], vec!["a^2".into()]), &["a", "b"], false)]);
    let s = layout(&f);
    assert!(s.draws.is_empty(), "{:?}", s.draws);
    assert_eq!(ntex_figures::build::figure_diagnostics(&f).len(), 1);
}

#[test]
fn grids_and_cubes_for_products() {
    let s = layout(&fig(vec![cayley("C", GroupSpec::Named("Z2xZ2xZ2".into()), &["a", "b", "c"], false)]));
    assert_eq!(s.anchors.iter().filter(|(o, _, _)| matches!(o, ObjRef::CayleyVertex(..))).count(), 8);
    assert_eq!(paths(&s, |o| matches!(o, ObjRef::CayleyEdge(..))).count(), 12, "a cube has 12 edges");
    let s = layout(&fig(vec![cayley("C", GroupSpec::Named("Z3xZ3".into()), &["a", "b"], false)]));
    assert_eq!(paths(&s, |o| matches!(o, ObjRef::CayleyEdge(..))).count(), 18);
}

#[test]
fn text_and_bounds() {
    let f = fig(vec![
        a("A", "B", Some("f")),
        Item::Text(Text { text: "kernel \\(\\mapsto\\) identity".into(), pos: Pos::Abs(1.0, -2.0), span: sp() }),
        Item::Blob(blob("G", "G", &["x"], None, false)),
        Item::NGon(ngon("P", 3)),
        cayley("C", GroupSpec::Named("Z3".into()), &["a"], false),
    ]);
    let s = layout(&f);
    assert!(s.draws.iter().any(|d| matches!(d, Draw::Label { obj: ObjRef::Text(_), math: false, .. })));
    let b = &s.bounds;
    for (_, p, r) in &s.anchors {
        assert!(p.x >= b.min.x && p.x <= b.max.x && p.y >= b.min.y && p.y <= b.max.y);
        assert!(r.min.x >= b.min.x - 1e-3 && r.max.x <= b.max.x + 1e-3 && r.min.y >= b.min.y - 1e-3 && r.max.y <= b.max.y + 1e-3);
    }
    for d in &s.draws {
        if let Draw::Path { start, segs, .. } = d {
            for q in flatten(*start, segs) {
                assert!(q.x >= b.min.x && q.x <= b.max.x && q.y >= b.min.y && q.y <= b.max.y);
            }
        }
    }
    // the groups don't overlap: blob, n-gon and Cayley graph left to right
    let x = |o: ObjRef| s.anchors.iter().find(|(x, _, _)| *x == o).map(|(_, _, r)| r.clone()).unwrap();
    assert!(x(ObjRef::Blob("G".into())).max.x < x(ObjRef::NGon("P".into())).min.x);
    assert!(x(ObjRef::NGon("P".into())).max.x < x(ObjRef::Cayley("C".into())).min.x);
}

#[test]
fn unknown_arrow_ends_become_nodes_and_vertices_resolve() {
    let f = fig(vec![Item::NGon(ngon("P", 4)), a("P.1", "P.3", Some("s")), a("X", "Y", None)]);
    let s = layout(&f);
    assert!(s.anchors.iter().any(|(o, _, _)| *o == ObjRef::Node("X".into())));
    assert!(!s.anchors.iter().any(|(o, _, _)| *o == ObjRef::Node("P.1".into())), "P.1 is a vertex, not a new node");
    assert!(paths(&s, |o| *o == ObjRef::Arrow(1)).next().is_some());
}

#[test]
fn tidy_snaps_straightens_and_evens() {
    let mut f = fig(vec![
        node("A", "A", Some(Pos::Cell(0.1, -0.2))),
        node("B", "B", Some(Pos::Abs(2.5, 0.1))),
        with(arr("A", "B", None), |a| a.bend = 5.0),
        with(arr("B", "A", None), |a| a.bend = 30.0),
        Item::Blob(Blob { pos: Some(Pos::Abs(0.0, 0.0)), ..blob("G", "G", &[], None, false) }),
        Item::Blob(Blob { pos: Some(Pos::Abs(3.0, 0.3)), ..blob("H", "H", &[], None, false) }),
        Item::Blob(Blob { pos: Some(Pos::Abs(8.0, -0.3)), ..blob("K", "K", &[], None, false) }),
    ]);
    let edits = tidy(&mut f);
    assert!(edits.contains(&Edit::MoveNode { name: "A".into(), pos: Pos::Cell(0.0, 0.0) }));
    assert!(edits.contains(&Edit::MoveNode { name: "B".into(), pos: Pos::Cell(1.0, 0.0) }));
    assert!(edits.iter().any(|e| matches!(e, Edit::SetArrow { index: 2, arrow } if arrow.bend == 0.0)));
    assert!(!edits.iter().any(|e| matches!(e, Edit::SetArrow { index: 3, .. })), "a real bend stays");
    assert!(edits.iter().any(|e| matches!(e, Edit::MoveBlob { name, pos: Pos::Abs(x, y) } if name == "H" && near(*x, 4.0, 0.01) && near(*y, 0.0, 0.01))));
    // idempotent
    assert!(tidy(&mut f).is_empty());
}
