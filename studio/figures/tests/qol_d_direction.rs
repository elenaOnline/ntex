//! figure-qol class D: direction and orientation. The shorthand for a
//! clockwise rotation arrow, its edge, negative turns, per-generator Cayley
//! looks (round trips), reversing an arrow, and the layout that follows.
mod core_a_common;
use core_a_common::*;
use ntex_figures::edit::{reversed, reversed_in};
use ntex_figures::layout::{self, layout};

fn ngon1(src: &str) -> NGon {
    let (f, d) = p(src);
    assert!(!d.iter().any(|x| x.severity == Severity::Error || x.severity == Severity::Warning), "{src}: {d:#?}");
    ngons(&f)[0].clone()
}

#[test]
fn rotation_shorthand_round_trips() {
    // old files: counter-clockwise, as before
    let g = ngon1("ngon P 5 rotation");
    assert!(g.rotation == 1 && !g.rotation_cw && g.rotation_edge.is_none());
    let g = ngon1("ngon P 5 rotation=2");
    assert!(g.rotation == 2 && !g.rotation_cw);
    // clockwise: `rotation cw`, `rotation=-k`
    let g = ngon1("ngon P 5 rotation cw");
    assert!(g.rotation == 1 && g.rotation_cw);
    let g = ngon1("ngon P 5 rotation=-1");
    assert!(g.rotation == 1 && g.rotation_cw);
    assert_eq!(format(&parse("ngon P 5 rotation=-1").0), "ngon P 5 rotation cw\n");
    let g = ngon1("ngon P 5 rotation=-2");
    assert!(g.rotation == 2 && g.rotation_cw);
    assert_eq!(format(&parse("ngon P 5 rotation=2 cw").0), "ngon P 5 rotation=-2\n");
    assert_eq!(format(&parse("ngon P 5 rotation ccw").0), "ngon P 5 rotation\n");
    // the edge
    let g = ngon1("ngon P 5 rotation cw edge=3");
    assert!(g.rotation_cw && g.rotation_edge == Some(3));
    assert_eq!(format(&parse("ngon P 5 rotation edge 3").0), "ngon P 5 rotation edge=3\n");
    let g = ngon1("ngon P 6 axes rotation=-2 edge=6 turn=1 color=maroon");
    assert!(g.rotation == 2 && g.rotation_cw && g.rotation_edge == Some(6) && g.turn == 1);
    // an edge past n wraps; an edge with no arrow is dropped
    assert_eq!(ngon1("ngon P 4 rotation edge=6").rotation_edge, Some(2));
    assert_eq!(parse("ngon P 4 rotation=0 edge=2").0.items.iter().find_map(|i| if let Item::NGon(g) = i { Some(g.rotation_edge) } else { None }), Some(None));
}

#[test]
fn turn_takes_negatives() {
    let g = ngon1("ngon P 4 turn=-1");
    assert_eq!((g.turn, g.turn_neg), (3, true), "-1 is 3 mod 4, written back as -1");
    assert_eq!(format(&parse("ngon P 4 turn=-1").0), "ngon P 4 turn=-1\n");
    let g = ngon1("ngon P 6 turn=-2");
    assert_eq!((g.turn, g.turn_neg), (4, true));
    assert_eq!(format(&parse("ngon P 6 turn=-2").0), "ngon P 6 turn=-2\n");
    let g = ngon1("ngon P 4 turn=1");
    assert_eq!((g.turn, g.turn_neg), (1, false));
    assert_eq!(ngon1("ngon P 4 turn=-4").turn, 0);
    assert_eq!(ngon1("ngon P 4 turn=3").turn, 3, "an old positive turn is unchanged");
    // the same relabelling as the positive turn it equals
    let a = layout::ngon_labels_at(&ngon1("ngon P 4 turn=-1"));
    let b = layout::ngon_labels_at(&ngon1("ngon P 4 turn=3"));
    assert_eq!(a, b);
    assert_eq!(a, vec!["2", "3", "4", "1"]);
}

#[test]
fn old_json_without_the_new_fields_still_loads() {
    let (f, _) = parse("ngon P 5 rotation=2 turn=1\ncayley D4 legend");
    let mut v = serde_json::to_value(&f).unwrap();
    for it in v["items"].as_array_mut().unwrap() {
        if let Some(g) = it.get_mut("NGon") {
            for k in ["rotation_cw", "rotation_edge", "turn_neg"] {
                g.as_object_mut().unwrap().remove(k);
            }
        }
        if let Some(c) = it.get_mut("Cayley") {
            c.as_object_mut().unwrap().remove("gen_styles");
        }
    }
    let g: Figure = serde_json::from_value(v).unwrap();
    assert_eq!(without_spans(&f), without_spans(&g));
}

#[test]
fn rotation_arrow_layout_direction_edge_and_label() {
    let sc = |src: &str| layout(&parse(src).0);
    let arc = |s: &Scene| {
        s.draws
            .iter()
            .find_map(|d| match d {
                Draw::Path { obj: ObjRef::NGonRotation(_), start, segs, end_tip: Some(_), .. } => Some((*start, match segs.last() { Some(Seg::Cubic(_, _, e)) | Some(Seg::Line(e)) => *e, None => *start })),
                _ => None,
            })
            .expect("a rotation arrow")
    };
    let label = |s: &Scene| s.draws.iter().find_map(|d| match d { Draw::Label { obj: ObjRef::NGonRotation(_), latex, at, .. } => Some((latex.clone(), *at)), _ => None }).expect("its label");
    let ccw = sc("ngon P 5 rotation");
    let cw = sc("ngon P 5 rotation cw");
    let (s0, e0) = arc(&ccw);
    let (s1, e1) = arc(&cw);
    // the same arc, the other way round
    assert!(dist(s0, e1) < 1e-3 && dist(e0, s1) < 1e-3, "clockwise is the same arc run backwards: {s0:?} {e0:?} / {s1:?} {e1:?}");
    // the ccw head goes anticlockwise round the centre
    let c = anchor_centre(&ccw, "P");
    let cross = |a: Pt, b: Pt| (a.x - c.x) * (b.y - c.y) - (a.y - c.y) * (b.x - c.x);
    assert!(cross(s0, e0) > 0.0 && cross(s1, e1) < 0.0);
    assert_eq!(label(&ccw).0, "r");
    assert_eq!(label(&cw).0, "r^{-1}");
    assert_eq!(label(&sc("ngon P 5 rotation=2")).0, "r^{2}");
    assert_eq!(label(&sc("ngon P 5 rotation=-2")).0, "r^{-2}");
    // its edge: default is the upper right; edge=3 on a pentagon moves it
    let dflt = label(&ccw).1;
    let e3 = label(&sc("ngon P 5 rotation edge=3")).1;
    assert!(dist(dflt, e3) > 1.0, "a different edge");
    // vertex 3-4's midpoint direction (pentagon, vertex 1 on top)
    let want = layout::rotation_edge_angle(5, Some(3));
    let got = (e3.y - c.y).atan2(e3.x - c.x).to_degrees();
    assert!(((want - got + 540.0).rem_euclid(360.0) - 180.0).abs() < 1.0, "{want} vs {got}");
    // the default edge is what it always was
    assert_eq!(layout::rotation_default_edge(5), 5, "the edge between vertices 5 and 1");
    assert_eq!(layout::nearest_edge(5, want, 0.0), 3);
    assert_eq!(layout::nearest_edge(4, layout::rotation_edge_angle(4, Some(2)) + 8.0, 0.0), 2);
}

fn dist(a: Pt, b: Pt) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}
fn anchor_centre(s: &Scene, name: &str) -> Pt {
    s.anchors.iter().find(|(o, _, _)| *o == ObjRef::NGon(name.into())).map(|(_, p, _)| *p).unwrap()
}

#[test]
fn cayley_generator_looks_round_trip() {
    let src = "cayley D4 <r, s> gens r:maroon,s:hidden";
    let (f, d) = p(src);
    assert!(d.iter().all(|x| x.severity == Severity::Note), "{d:#?}");
    let c = &cayleys(&f)[0];
    assert_eq!(c.gen_styles.len(), 2);
    assert_eq!(c.gen_styles[0], GenStyle { generator: "r".into(), color: Some("maroon".into()), inverse: false, hidden: false });
    assert!(c.gen_styles[1].hidden);
    assert_eq!(format(&f), "cayley D4 <r, s> gens r:maroon,s:hidden\n");
    let (f, _) = p("cayley D4 <r, s> gens r^-1:teal  legend");
    let c = &cayleys(&f)[0];
    assert_eq!(c.gen_styles, vec![GenStyle { generator: "r".into(), color: Some("teal".into()), inverse: true, hidden: false }]);
    assert_eq!(format(&f), "cayley D4 <r, s> gens r^-1:teal legend\n");
    // spaced list, a plain `r` (no entry), the default generators (no <…>)
    let (f, _) = p("cayley D3 gens r, s:green");
    assert_eq!(cayleys(&f)[0].gen_styles.len(), 1);
    assert_eq!(format(&f), "cayley D3 gens s:green\n");
    // a presentation, unicode generators as she spelled them
    let (f, _) = p("cayley <a, b | a^4, b^2, (ab)^2> gens a^-1:blue");
    assert_eq!(cayleys(&f)[0].gen_styles[0].generator, "a");
    let (f, d) = p("cayley <ρ, σ | ρ^3, σ^2, (ρσ)^2> gens ρ^-1");
    assert!(d.iter().all(|x| x.severity == Severity::Note), "{d:#?}");
    assert!(format(&f).contains("gens ρ^-1"), "{}", format(&f));
    // a bad colour is a warning, not a loss of the rest
    let (f, d) = parse("cayley D4 <r, s> gens r:plaid,s:hidden");
    assert!(d.iter().any(|x| x.severity == Severity::Warning));
    assert_eq!(cayleys(&f)[0].gen_styles.len(), 1, "the bad colour leaves r plain; s:hidden stays");
}

#[test]
fn cayley_edges_follow_the_generator_looks() {
    let count = |s: &Scene, color: &str| s.draws.iter().filter(|d| matches!(d, Draw::Path { obj: ObjRef::CayleyEdge(..), stroke: Some(st), .. } if st.color == color)).count();
    let edges = |s: &Scene| s.draws.iter().filter(|d| matches!(d, Draw::Path { obj: ObjRef::CayleyEdge(..), .. })).count();
    let base = layout(&parse("cayley D4 <r, s>").0);
    let n_all = edges(&base);
    assert!(n_all > 8);
    // recolour r
    let col = layout(&parse("cayley D4 <r, s> gens r:violet").0);
    assert_eq!(edges(&col), n_all);
    assert_eq!(count(&col, "violet"), count(&base, "blue"));
    assert_eq!(count(&col, "blue"), 0);
    // hide s: only r's edges remain (an involution's edges are drawn once)
    let hid = layout(&parse("cayley D4 <r, s> gens s:hidden").0);
    assert_eq!(edges(&hid), count(&base, "blue"));
    // r⁻¹: the same edges, every arrow pointing the other way
    let first = |s: &Scene| s.draws.iter().find_map(|d| match d { Draw::Path { obj: ObjRef::CayleyEdge(..), start, segs, end_tip: Some(_), .. } => Some((*start, match segs.last() { Some(Seg::Cubic(_, _, e)) | Some(Seg::Line(e)) => *e, None => *start })), _ => None }).unwrap();
    let inv = layout(&parse("cayley D4 <r, s> gens r^-1").0);
    let (a, b) = first(&base);
    let (c, d) = first(&inv);
    assert!(dist(a, d) < 0.3 && dist(b, c) < 0.3, "reversed: {a:?}->{b:?} vs {c:?}->{d:?}");
    // the legend follows: hidden generators leave it, r⁻¹ is named
    let leg = layout(&parse("cayley D4 <r, s> legend gens s:hidden,r^-1").0);
    let labels: Vec<String> = leg.draws.iter().filter_map(|d| match d { Draw::Label { obj: ObjRef::Cayley(_), latex, .. } => Some(latex.clone()), _ => None }).collect();
    assert_eq!(labels, vec!["{r}^{-1}".to_string()]);
}

#[test]
fn reversing_an_arrow() {
    let (f, _) = p("A at 0,0\nB at 1,0\nA -f-> B bend-left near-start\nA -g-> B swap\nA -h-hook-> B\nA -i-> B near-end");
    let arr = arrows(&f);
    let r = reversed(arr[0]);
    assert_eq!((r.from.as_str(), r.to.as_str()), ("B", "A"));
    assert_eq!(r.side, LabelSide::Swap, "left of the new direction is the old right");
    assert_eq!(r.bend, -arr[0].bend);
    assert_eq!(r.label_pos, LabelPos::NearEnd);
    assert_eq!(r.label.as_deref(), Some("f"));
    assert_eq!(reversed(arr[1]).side, LabelSide::Auto);
    assert_eq!(reversed(arr[2]).head, Head::Hook, "the hook stays at the arrow's tail end: now B's");
    assert_eq!(reversed(arr[3]).label_pos, LabelPos::NearStart);
    // twice is the identity
    assert_eq!(reversed(&reversed(arr[0])), *arr[0]);
    // the label stays where it was drawn: same side of the same line, same spot
    let drawn = |a: &Arrow| {
        let mut g = f.clone();
        let i = g.items.iter().position(|it| matches!(it, Item::Arrow(x) if x.label == a.label && x.to == "B" || x.label == a.label && x.to == "A")).unwrap();
        let _ = i;
        g.items.retain(|it| !matches!(it, Item::Arrow(_)));
        g.items.push(Item::Arrow(a.clone()));
        let s = layout(&g);
        s.draws.iter().find_map(|d| match d { Draw::Label { obj: ObjRef::Arrow(_), at, .. } => Some(*at), _ => None }).unwrap()
    };
    for a in [arr[0], arr[1], arr[3]] {
        let (p0, p1) = (drawn(a), drawn(&reversed(a)));
        assert!(dist(p0, p1) < 0.12, "the label of {} stays: {p0:?} vs {p1:?}", a.label.clone().unwrap());
    }
    // blob to blob: the layout already keeps the label above, `swap` stays
    let (g, _) = p("blob G {e}\nblob H {e'}\nG -f-> H");
    let a = arrows(&g)[0].clone();
    assert_eq!(reversed_in(&g, &a).side, a.side);
    assert_eq!(reversed(&a).side, LabelSide::Swap);
    let (l, _) = p("A\nA -f-> A");
    let a = arrows(&l)[0].clone();
    assert_eq!(reversed(&a), a, "a loop has no direction to turn");
}

#[test]
fn reversing_through_set_item_keeps_the_spelling() {
    let (mut f, _) = p("A at 0,0\nB at 1,0\nA -φ-> B");
    let i = f.items.iter().position(|it| matches!(it, Item::Arrow(_))).unwrap();
    let Item::Arrow(a) = f.items[i].clone() else { unreachable!() };
    let r = reversed(&a);
    ntex_figures::edit::apply(&mut f, &Edit::SetItem { index: i, item: Item::Arrow(r) }).unwrap();
    let Item::Arrow(b) = &f.items[i] else { unreachable!() };
    assert_eq!((b.from.as_str(), b.to.as_str()), ("B", "A"));
    assert!(format(&f).contains("B -"), "{}", format(&f));
}

#[test]
fn set_item_normalizes_the_new_ngon_fields() {
    let (mut f, _) = p("ngon P 4 rotation cw edge=2 turn=-1");
    let Item::NGon(mut g) = f.items[0].clone() else { unreachable!() };
    g.rotation = 0;
    g.turn = 0;
    ntex_figures::edit::apply(&mut f, &Edit::SetItem { index: 0, item: Item::NGon(g) }).unwrap();
    let Item::NGon(g) = &f.items[0] else { unreachable!() };
    assert!(!g.rotation_cw && g.rotation_edge.is_none() && !g.turn_neg);
    // deleting the rotation arrow takes the arrow, not the polygon
    let (mut f, _) = p("ngon P 4 rotation cw edge=2");
    ntex_figures::edit::apply(&mut f, &Edit::Delete { obj: ObjRef::NGonRotation("P".into()) }).unwrap();
    assert_eq!(format(&f), "ngon P 4\n");
    // moving it is refused
    assert!(ntex_figures::edit::apply(&mut f, &Edit::MoveObj { obj: ObjRef::NGonRotation("P".into()), pos: Pos::Abs(0.0, 0.0) }).is_err());
}
