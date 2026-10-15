//! figure-qol class E: what the shorthand can say, the mouse can too.
//! Free text colour and size (round trips, layout, TikZ); auto-place.
mod core_a_common;
use core_a_common::*;
use ntex_figures::layout::layout;
use ntex_figures::tikz;

fn text1(src: &str) -> Text {
    let (f, d) = p(src);
    assert!(!d.iter().any(|x| x.severity == Severity::Error || x.severity == Severity::Warning), "{src}: {d:#?}");
    f.items.iter().find_map(|i| if let Item::Text(t) = i { Some(t.clone()) } else { None }).expect("a text")
}

#[test]
fn text_colour_and_size_round_trip() {
    let t = text1("text \"kernel\" at (0,-2)");
    assert!(t.color.is_none() && t.size.is_none());
    let t = text1("text \"kernel\" at (0,-2) color=teal size=large");
    assert_eq!(t.color.as_deref(), Some("teal"));
    assert_eq!(t.size.as_deref(), Some("large"));
    // any order, size normal is the default
    let t = text1("text \"x\" size=small at 1,0 color=maroon");
    assert_eq!((t.color.as_deref(), t.size.as_deref()), (Some("maroon"), Some("small")));
    assert!(text1("text \"x\" at 1,0 size=normal").size.is_none());
    for src in ["text \"kernel ↦ identity\" at (0,-2) color=teal size=large\n", "text \"a\" at 1,2 size=huge\n", "text \"b\" at (0,0) color=ochre\n"] {
        let once = format(&parse(src).0);
        assert_eq!(once, src, "her spelling is kept");
        assert_eq!(format(&parse(&once).0), once);
    }
    // an unknown colour is a warning (words stay words), not a crash
    let (f, d) = parse("text \"x\" at 1,0 color=pink");
    assert!(f.items.iter().any(|i| matches!(i, Item::Text(t) if t.color.is_none() && t.text == "x")));
    assert!(d.iter().any(|x| x.severity == Severity::Warning));
}

#[test]
fn text_colour_and_size_are_drawn() {
    let (f, _) = parse("text \"hello\" at (0,0) color=maroon size=huge\ntext \"plain\" at (0,-1)\n");
    let sc = layout(&f);
    let lab = |want: &str| {
        sc.draws.iter().find_map(|d| match d {
            Draw::Label { latex, color, size, .. } if latex == want => Some((color.clone(), *size)),
            _ => None,
        })
    };
    let (c1, s1) = lab("hello").unwrap();
    let (c2, s2) = lab("plain").unwrap();
    assert_eq!(c1, "maroon");
    assert_eq!(c2, "ink");
    assert!(s1 > s2 * 1.3, "{s1} vs {s2}");
    let tex = tikz::picture(&f, &sc);
    assert!(tex.contains("ntex maroon") && tex.contains("\\Large"), "{tex}");
}

#[test]
fn clearing_a_node_position_auto_places_it() {
    let (mut f, _) = parse("A at 0,0\nB at 3,0\nC at 0,-2\nA -> B ; A -> C\n");
    let before = format(&f);
    assert!(before.contains("C at 0,-2"));
    if let Some(Item::Node(n)) = f.items.iter_mut().find(|i| matches!(i, Item::Node(n) if n.name == "C")) {
        n.pos = None;
    }
    let after = format(&f);
    assert!(!after.contains("C at"), "{after}");
    let sc = layout(&f);
    assert!(sc.anchors.iter().any(|(o, _, _)| *o == ObjRef::Node("C".into())), "C is still placed");
    // and it re-parses to the same
    assert_eq!(format(&parse(&after).0), after);
}

#[test]
fn arrow_label_position_round_trips() {
    for (src, want) in [("A -f-> B near-start\n", LabelPos::NearStart), ("A -f-> B near-end\n", LabelPos::NearEnd)] {
        let (f, _) = parse(src);
        let a = f.items.iter().find_map(|i| if let Item::Arrow(a) = i { Some(a.clone()) } else { None }).unwrap();
        assert_eq!(a.label_pos, want);
        assert_eq!(format(&f), src);
    }
}
