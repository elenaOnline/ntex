//! (FIG-INTERACT) A `grid … end` block survives the window's edits that
//! its art can say: a relabel, a removed label, a vertical arrow's side,
//! solid ↔ dashed down a column, a deleted arrow, a node's colour or label.
//! A move still converts it.
use ntex_figures::edit::apply;
use ntex_figures::format::format;
use ntex_figures::model::*;
use ntex_figures::parse::parse;

const SQUARE: &str = "grid\n  A --f--> B\n  |g       |h\n  v        v\n  C --k--> D\nend\n";

fn fig() -> Figure {
    parse(SQUARE).0
}

fn arrow(f: &Figure, from: &str, to: &str) -> (usize, Arrow) {
    f.items.iter().enumerate().find_map(|(i, it)| match it {
        Item::Arrow(a) if a.from == from && a.to == to => Some((i, a.clone())),
        _ => None,
    }).unwrap()
}

/// format, parse back: the same objects, and the source still a grid.
fn check(f: &Figure, must: &[&str]) -> String {
    let src = format(f);
    assert!(src.contains("grid\n") && src.contains("\nend"), "the grid was converted:\n{src}");
    assert!(!src.contains("converted") && !src.contains("rewritten"), "the grid was converted:\n{src}");
    for m in must {
        assert!(src.contains(m), "expected `{m}` in:\n{src}");
    }
    let back = parse(&src).0;
    let objs = |f: &Figure| f.items.iter().filter(|i| matches!(i, Item::Node(_) | Item::Arrow(_))).map(|i| match i {
        Item::Node(n) => format!("{} {} {:?} {:?}", n.name, n.label, n.pos, n.color),
        Item::Arrow(a) => format!("{}>{} {:?} {:?} {:?} {:?}", a.from, a.to, a.label, a.side, a.line, a.head),
        _ => String::new(),
    }).collect::<Vec<_>>();
    let (mut x, mut y) = (objs(f), objs(&back));
    x.sort();
    y.sort();
    assert_eq!(x, y, "round trip:\n{src}");
    src
}

#[test]
fn relabel_horizontal_keeps_the_art() {
    let mut f = fig();
    let (i, mut a) = arrow(&f, "A", "B");
    a.label = Some("φ".into());
    apply(&mut f, &Edit::SetArrow { index: i, arrow: a }).unwrap();
    check(&f, &["A --φ--> B"]);
}

#[test]
fn longer_label_keeps_the_art() {
    let mut f = fig();
    let (i, mut a) = arrow(&f, "A", "B");
    a.label = Some("\\varphi".into());
    apply(&mut f, &Edit::SetArrow { index: i, arrow: a }).unwrap();
    check(&f, &["\\varphi"]);
}

#[test]
fn removed_label_keeps_the_art() {
    let mut f = fig();
    let (i, mut a) = arrow(&f, "A", "B");
    a.label = None;
    apply(&mut f, &Edit::SetArrow { index: i, arrow: a }).unwrap();
    check(&f, &[]);
}

#[test]
fn relabel_vertical_keeps_the_art() {
    let mut f = fig();
    let (i, mut a) = arrow(&f, "A", "C");
    a.label = Some("ψ".into());
    apply(&mut f, &Edit::SetItem { index: i, item: Item::Arrow(a) }).unwrap();
    check(&f, &["|ψ"]);
}

#[test]
fn vertical_swap_keeps_the_art() {
    let mut f = fig();
    let (i, mut a) = arrow(&f, "A", "C");
    a.side = LabelSide::Swap;
    apply(&mut f, &Edit::SetItem { index: i, item: Item::Arrow(a) }).unwrap();
    check(&f, &["g|"]);
}

#[test]
fn vertical_dashed_keeps_the_art() {
    let mut f = fig();
    let (i, mut a) = arrow(&f, "B", "D");
    a.line = Line::Dashed;
    apply(&mut f, &Edit::SetItem { index: i, item: Item::Arrow(a) }).unwrap();
    check(&f, &[":h"]);
}

#[test]
fn deleting_an_arrow_keeps_the_art() {
    let mut f = fig();
    let (i, _) = arrow(&f, "B", "D");
    apply(&mut f, &Edit::Delete { obj: ObjRef::Arrow(i) }).unwrap();
    let src = check(&f, &[]);
    assert!(!src.contains("|h"), "{src}");
    let (i, _) = arrow(&f, "A", "B");
    apply(&mut f, &Edit::Delete { obj: ObjRef::Arrow(i) }).unwrap();
    check(&f, &[]);
}

#[test]
fn node_colour_and_label_keep_the_art() {
    let mut f = fig();
    let i = f.items.iter().position(|it| matches!(it, Item::Node(n) if n.name == "A")).unwrap();
    let Item::Node(mut n) = f.items[i].clone() else { unreachable!() };
    n.color = Some("blue".into());
    apply(&mut f, &Edit::SetItem { index: i, item: Item::Node(n) }).unwrap();
    apply(&mut f, &Edit::SetNodeLabel { name: "B".into(), label: "ℤ".into() }).unwrap();
    check(&f, &["color=blue", "ℤ"]);
}

#[test]
fn a_move_still_converts() {
    let mut f = fig();
    apply(&mut f, &Edit::MoveNode { name: "A".into(), pos: Pos::Cell(0.0, 3.0) }).unwrap();
    let src = format(&f);
    assert!(!src.contains("\ngrid\n") && !src.starts_with("grid"), "{src}");
}

