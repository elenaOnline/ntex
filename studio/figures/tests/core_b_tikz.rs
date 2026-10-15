//! Scene → TikZ (FIG-CORE-B): tikz-cd arrow styles, label sides, palette
//! names, exact coordinates, portable output.
mod core_b_common;
use core_b_common::*;
use ntex_figures::layout::layout;
use ntex_figures::model::*;
use ntex_figures::tikz::*;

fn square() -> Figure {
    fig(vec![
        node("A", "A", Some(Pos::Cell(0.0, 0.0))),
        node("B", "B", Some(Pos::Cell(1.0, 0.0))),
        node("C", "C", Some(Pos::Cell(0.0, 1.0))),
        node("D", "D", Some(Pos::Cell(1.0, 1.0))),
        a("A", "B", Some("f")),
        with(arr("A", "C", Some("g")), |a| a.side = LabelSide::Swap),
        with(arr("B", "D", Some("h")), |a| a.label_pos = LabelPos::NearEnd),
        with(arr("C", "D", Some("k")), |a| a.color = Some("blue".into())),
    ])
}

#[test]
fn nodes_are_named_cells_at_exact_scene_points() {
    let f = square();
    let s = layout(&f);
    let t = picture(&f, &s);
    assert!(t.starts_with("\\begin{tikzpicture}[ntex figure]"));
    assert!(t.trim_end().ends_with("\\end{tikzpicture}"));
    assert!(t.contains("\\node[commutative diagrams/every cell] (n1) at (0,0) {$A$};"));
    assert!(t.contains("(n2) at (1.9,0) {$B$}"));
    assert!(t.contains("(n3) at (0,-1.35) {$C$}"));
    // the bounding box is the Scene's
    let bb = format!("\\useasboundingbox ({},{}) rectangle", trim(s.bounds.min.x), trim(s.bounds.min.y));
    assert!(t.contains(&bb), "{t}");
    // arrows run between the named nodes with tikz-cd's styles
    assert!(t.contains("\\draw[commutative diagrams/.cd, every arrow, every label, rightarrow, /tikz/.cd] (n1) -- node {$f$} (n2);"), "{t}");
    assert!(t.contains("(n1) -- node[swap] {$g$} (n3)"), "swap → tikz swap: {t}");
    assert!(t.contains("node[pos=0.75] {$h$}"), "near end: {t}");
    assert!(t.contains("ntex blue] (n3) -- node {$k$} (n4)"), "palette colour by name: {t}");
}

fn trim(v: f32) -> String {
    let v = if v.abs() < 0.0005 { 0.0 } else { v };
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s == "-0" { "0".into() } else { s }
}

#[test]
fn every_head_and_line_maps_to_tikz_cd() {
    let heads = [
        (Head::To, Line::Solid, "rightarrow"),
        (Head::TwoHeads, Line::Solid, "two heads"),
        (Head::Tail, Line::Solid, "tail"),
        (Head::Hook, Line::Solid, "hook"),
        (Head::MapsTo, Line::Solid, "maps to"),
        (Head::Both, Line::Solid, "leftrightarrow"),
        (Head::Equal, Line::Solid, "equal"),
        (Head::Double, Line::Solid, "Rightarrow"),
        (Head::To, Line::Dashed, "rightarrow, /tikz/.cd, dashed"),
        (Head::To, Line::Dotted, "rightarrow, /tikz/.cd, dotted"),
    ];
    for (h, l, key) in heads {
        let f = fig(vec![node("A", "A", Some(Pos::Cell(0.0, 0.0))), node("B", "B", Some(Pos::Cell(1.0, 0.0))), with(arr("A", "B", None), |a| {
            a.head = h;
            a.line = l
        })]);
        let t = picture(&f, &layout(&f));
        assert!(t.contains(&format!("every label, {key}")), "{h:?} {l:?} → {key}: {t}");
    }
    // bends are explicit curves that still start and end at the named nodes
    let f = fig(vec![node("A", "A", Some(Pos::Cell(0.0, 0.0))), node("B", "B", Some(Pos::Cell(1.0, 0.0))), with(arr("A", "B", Some("f")), |a| a.bend = 30.0)]);
    let t = picture(&f, &layout(&f));
    assert!(t.contains("(n1) .. controls ("), "{t}");
    assert!(t.contains(".. node {$f$} (n2);"), "{t}");
    // iso: ≅ on the other side
    let f = fig(vec![node("A", "A", Some(Pos::Cell(0.0, 0.0))), node("B", "B", Some(Pos::Cell(1.0, 0.0))), with(arr("A", "B", Some("f")), |a| a.iso = true)]);
    let t = picture(&f, &layout(&f));
    assert!(t.contains("node {$f$} node[swap] {$\\cong$}"), "{t}");
}

#[test]
fn blobs_ngons_cayley_emit_their_scene_geometry() {
    let f = fig(vec![
        Item::Blob(blob("G", "G", &["g"], None, false)),
        Item::Blob(blob("K", "K", &["e"], Some("G"), true)),
        Item::NGon(NGon { all_axes: true, rotation: 1, ..ngon("P", 4) }),
        cayley("C", GroupSpec::Named("D3".into()), &["r", "s"], true),
    ]);
    let s = layout(&f);
    let t = picture(&f, &s);
    assert!(t.contains("fill=ntexFigBlueTint] ") && t.contains("% blob K"), "a shaded blob");
    assert!(t.contains(".. cycle; % blob G"), "a closed smooth curve");
    assert!(t.contains("-- cycle; % n-gon P"), "a closed polygon");
    assert!(t.contains("dash pattern=on 2.4pt off 1.8pt"), "dashed axes");
    assert!(t.contains("ntex maroon"), "the rotation arrow");
    assert!(t.contains("circle[radius=1.6pt]"), "element dots");
    // Cayley vertices are named nodes and edges run between them
    assert!(t.contains("(v1) at ") && t.contains("{$e$}; % C.e"));
    assert!(t.contains("dash, /tikz/.cd, ntex maroon") || t.contains("dash, /tikz/.cd, ntex maroon,"), "s is an involution: no head: {t}");
    assert!(t.contains("rightarrow, /tikz/.cd, ntex blue, line width=0.6pt] (v"), "r edges between named vertices: {t}");
    // every coordinate in the TikZ is one the Scene has (no rounding beyond 1e-3)
    for d in &s.draws {
        if let Draw::Dot { at, .. } = d {
            assert!(t.contains(&format!("({},{}) circle", trim(at.x), trim(at.y))));
        }
    }
}

#[test]
fn document_and_portable() {
    let f = square();
    let s = layout(&f);
    let d = document(&f, &s);
    assert!(d.contains("\\documentclass[border=3pt]{standalone}"));
    assert!(d.contains("\\usepackage{ntex-figure}"));
    assert!(d.contains(&picture(&f, &s)));
    let p = portable(&f, &s);
    assert!(!p.lines().any(|l| l.starts_with("\\usepackage")), "no packages in a paste-in snippet");
    assert!(p.contains("\\definecolor{ntexFigBlue}{HTML}{1E47B8}"));
    assert!(p.contains("ntex blue/.style={color=ntexFigBlue}"));
    assert!(p.contains("ntex figure/.style="));
    assert!(p.contains(&picture(&f, &s)));
    assert_eq!(color_name("teal-tint"), "ntexFigTealTint");
    assert_eq!(color_name("nonsense"), "ntexFigInk");
}
