//! figure-qol class A: cloning objects (⌥-drag, ⌘D, ⌘C/⌘V), the clipboard's
//! shorthand and pasting text from elsewhere.
mod core_a_common;
use core_a_common::*;
use ntex_figures::clip::{copy_objects, from_shorthand, paste_into, to_shorthand};

fn fig(src: &str) -> Figure {
    let (f, d) = p(src);
    assert!(!d.iter().any(|x| x.severity == Severity::Error), "{src}: {d:#?}");
    f
}

fn node<'a>(f: &'a Figure, name: &str) -> &'a Node {
    f.items.iter().find_map(|i| if let Item::Node(n) = i { (n.name == name).then_some(n) } else { None }).unwrap_or_else(|| panic!("no node {name}"))
}

#[test]
fn a_copy_brings_the_arrows_between_the_copied_objects() {
    let src = "A at 0,0\nB at 2,0\nC at 4,0\nA -f-> B\nB -g-> C\n";
    let f = fig(src);
    let mut g = f.clone();
    let objs = vec![ObjRef::Node("A".into()), ObjRef::Node("B".into())];
    let got = copy_objects(&mut g, &f, &objs, Pt { x: 1.9, y: -1.35 }, false).unwrap();
    // two new nodes with fresh names, the same labels, one cell right and down
    let new: Vec<String> = got.pairs.iter().filter_map(|(_, n)| if let ObjRef::Node(n) = n { Some(n.clone()) } else { None }).collect();
    assert_eq!(new.len(), 2);
    assert!(new.iter().all(|n| !["A", "B", "C"].contains(&n.as_str())));
    let a2 = node(&g, &new[0]);
    assert_eq!(a2.label, "A");
    assert_eq!(a2.pos, Some(Pos::Cell(1.0, 1.0)));
    // the arrow A→B comes along, renamed; B→C (C not copied) does not
    let arrows: Vec<&Arrow> = g.items.iter().filter_map(|i| if let Item::Arrow(a) = i { Some(a) } else { None }).collect();
    assert_eq!(arrows.len(), 3);
    let copied = arrows[2];
    assert_eq!((copied.from.as_str(), copied.to.as_str()), (new[0].as_str(), new[1].as_str()));
    assert_eq!(copied.label.as_deref(), Some("f"));
    assert_eq!(got.arrows.len(), 1);
    // and the result is a figure that survives the round trip
    let text = format(&g);
    assert_eq!(without_spans(&parse(&text).0), without_spans(&g));
}

#[test]
fn a_lone_arrow_is_not_copied_without_its_ends() {
    let f = fig("A at (0,0)\nB at (2,0)\nA -> B\n");
    let mut g = f.clone();
    let i = f.items.iter().position(|i| matches!(i, Item::Arrow(_))).unwrap();
    let got = copy_objects(&mut g, &f, &[ObjRef::Arrow(i)], Pt::default(), false).unwrap();
    assert_eq!(got.skipped_arrows, 1);
    assert_eq!(g.items.len(), f.items.len());
}

#[test]
fn auto_placed_nodes_are_copied_where_they_are_drawn() {
    let f = fig("A -> B\n");
    let mut g = f.clone();
    let got = copy_objects(&mut g, &f, &[ObjRef::Node("A".into())], Pt { x: 0.0, y: 0.0 }, false).unwrap();
    let ObjRef::Node(n) = &got.pairs[0].1 else { panic!() };
    // A sits at cell (0,0); its copy is pinned there
    assert_eq!(node(&g, n).pos, Some(Pos::Cell(0.0, 0.0)));
}

#[test]
fn shapes_blobs_and_text_copy_with_their_looks() {
    let src = "ngon P 5 axes at (1,1) color=maroon\nblob G {e, g} at (6,0) size=(2,1.5)\ntext \"hi\" at (0,-3)\nP.1 -> P.3\n";
    let f = fig(src);
    let objs = vec![ObjRef::NGon("P".into()), ObjRef::Blob("G".into()), ObjRef::Text(2)];
    let mut g = f.clone();
    let got = copy_objects(&mut g, &f, &objs, Pt { x: 1.0, y: -1.0 }, false).unwrap();
    assert_eq!(got.pairs.len(), 3);
    let ng = ngons(&g);
    assert_eq!(ng.len(), 2);
    assert_eq!(ng[1].color.as_deref(), Some("maroon"));
    assert_eq!(ng[1].pos, Some(Pos::Abs(2.0, 0.0)));
    assert_ne!(ng[1].name, "P");
    // the arrow between P's own vertices follows its copy
    let a: Vec<&Arrow> = g.items.iter().filter_map(|i| if let Item::Arrow(a) = i { Some(a) } else { None }).collect();
    assert_eq!(a.len(), 2);
    assert_eq!(a[1].from, format!("{}.1", ng[1].name));
    let blobs: Vec<&Blob> = g.items.iter().filter_map(|i| if let Item::Blob(b) = i { Some(b) } else { None }).collect();
    assert_eq!(blobs.len(), 2);
    assert_eq!(blobs[1].elements.len(), 2);
    assert!(blobs[1].elements.iter().all(|e| e.name != "e" && e.name != "g"));
    assert_eq!(blobs[1].pos, Some(Pos::Abs(7.0, -1.0)));
    let texts: Vec<&Text> = g.items.iter().filter_map(|i| if let Item::Text(t) = i { Some(t) } else { None }).collect();
    assert_eq!(texts[1].pos, Pos::Abs(1.0, -4.0));
    let text = format(&g);
    assert_eq!(without_spans(&parse(&text).0), without_spans(&g));
}

#[test]
fn her_spelling_carries_to_the_copy() {
    let f = fig("A \"φ\" at (0,0)\nB at (2,0)\nA -\"ψ\"-> B\n");
    let mut g = f.clone();
    copy_objects(&mut g, &f, &[ObjRef::Node("A".into()), ObjRef::Node("B".into())], Pt { x: 0.0, y: -1.35 }, false).unwrap();
    let text = format(&g);
    assert_eq!(text.matches("φ").count(), 2, "{text}");
    assert_eq!(text.matches("ψ").count(), 2, "{text}");
}

#[test]
fn the_clipboard_holds_shorthand_that_pastes_back() {
    let f = fig("A at 0,0\nB at 2,0\nC at 4,0\nA -f-> B bend-left\nB -> C\n");
    let text = to_shorthand(&f, &[ObjRef::Node("A".into()), ObjRef::Node("B".into())]).unwrap();
    // names kept; only what was copied, with its arrow
    assert!(text.contains("A") && text.contains("B") && !text.contains('C'), "{text}");
    assert!(text.contains("bend-left"), "{text}");
    // into another figure: the names are free, so they stay
    let mut other = fig("X at (0,0)\n");
    let got = paste_into(&mut other, &text, Pt::default()).unwrap();
    assert_eq!(got.pairs.len(), 2);
    assert!(other.items.iter().any(|i| matches!(i, Item::Node(n) if n.name == "A")));
    // into the same figure: fresh names
    let mut same = f.clone();
    let got = paste_into(&mut same, &text, Pt { x: 0.0, y: -1.35 }).unwrap();
    for (_, n) in &got.pairs {
        let ObjRef::Node(n) = n else { panic!() };
        assert!(!["A", "B", "C"].contains(&n.as_str()), "{n}");
    }
    assert_eq!(same.items.iter().filter(|i| matches!(i, Item::Arrow(_))).count(), 3);
}

#[test]
fn text_that_is_not_shorthand_is_refused() {
    assert!(from_shorthand("").is_err());
    assert!(from_shorthand("   \n").is_err());
    assert!(from_shorthand("% only a comment\n").is_err());
    let mut f = fig("A at (0,0)\n");
    assert!(paste_into(&mut f, "Dear Bob, thanks for the notes. See you at 5.", Pt::default()).is_err());
    assert!(paste_into(&mut f, "hello world", Pt::default()).is_err());
    assert!(paste_into(&mut f, "\\begin{tikzcd} A \\arrow[r] & B \\end{tikzcd}", Pt::default()).is_err());
    assert_eq!(f.items.len(), 1);
    // real shorthand with only a bit of syntax works
    assert!(from_shorthand("A -> B\n").is_ok());
    assert!(from_shorthand("ngon P 4 rotation\n").is_ok());
}
