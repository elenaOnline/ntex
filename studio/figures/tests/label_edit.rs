//! (figure-qol class C) Labels for the parts that had none to set: the
//! rotation arrow's, an axis's, a Cayley vertex's. Shorthand round-trips,
//! her spelling is kept, and the scene draws them where they belong.
use ntex_figures::format::format;
use ntex_figures::layout::layout;
use ntex_figures::model::*;
use ntex_figures::parse::parse;

fn ngon(f: &Figure) -> NGon {
    f.items.iter().find_map(|i| if let Item::NGon(g) = i { Some(g.clone()) } else { None }).unwrap()
}
fn cayley(f: &Figure) -> Cayley {
    f.items.iter().find_map(|i| if let Item::Cayley(c) = i { Some(c.clone()) } else { None }).unwrap()
}

/// parse → format → parse: the same figure items, and the text is stable.
fn round_trip(src: &str) -> (Figure, String) {
    let (f, d) = parse(src);
    assert!(d.iter().all(|x| x.severity != Severity::Error), "{d:?}");
    let out = format(&f);
    let (g, d2) = parse(&out);
    assert!(d2.iter().all(|x| x.severity != Severity::Error), "{d2:?}\n{out}");
    assert_eq!(f.items.iter().filter(|i| matches!(i, Item::NGon(_) | Item::Cayley(_))).count(), g.items.iter().filter(|i| matches!(i, Item::NGon(_) | Item::Cayley(_))).count());
    for (a, b) in f.items.iter().zip(g.items.iter()) {
        match (a, b) {
            (Item::NGon(x), Item::NGon(y)) => {
                assert_eq!((&x.rotation, &x.rotation_label, &x.axes, &x.axis_labels), (&y.rotation, &y.rotation_label, &y.axes, &y.axis_labels), "{out}");
            }
            (Item::Cayley(x), Item::Cayley(y)) => assert_eq!(x.vertex_names, y.vertex_names, "{out}"),
            _ => {}
        }
    }
    assert_eq!(format(&g), out, "format is stable");
    (f, out)
}

#[test]
fn axis_labels_round_trip() {
    let (f, out) = round_trip("ngon P 4 axis 1 \"s\" axis 2.5 \"t\" axis 2\n");
    let g = ngon(&f);
    assert_eq!(g.axes, vec![2, 5, 4]);
    assert_eq!(g.axis_labels, vec![(2, "s".to_string()), (5, "t".to_string())]);
    assert!(out.contains("axis 1 \"s\"") && out.contains("axis 2.5 \"t\"") && out.contains(" axis 2"), "{out}");
}

#[test]
fn rotation_label_round_trips_with_her_spelling() {
    let (f, out) = round_trip("ngon P 5 rotation \"ρ\"\nngon Q 6 rotation=2 \"ρ^2\"\n");
    let gs: Vec<NGon> = f.items.iter().filter_map(|i| if let Item::NGon(g) = i { Some(g.clone()) } else { None }).collect();
    assert_eq!(gs[0].rotation_label.as_deref(), Some(r"\rho"));
    assert_eq!(gs[1].rotation, 2);
    assert!(out.contains("rotation \"ρ\"") && out.contains("rotation=2 \"ρ^2\""), "her ρ stays ρ:\n{out}");
}

#[test]
fn cayley_vertex_names_round_trip() {
    let (f, out) = round_trip("cayley D4 <r, s> vertex r^2 \"ρ²\" vertex rs \"x\" angle=10\n");
    let c = cayley(&f);
    assert_eq!(c.vertex_names.len(), 2);
    assert!(out.contains("vertex r^2 \"ρ²\"") && out.contains("vertex rs \"x\""), "{out}");
}

#[test]
fn the_defaults_write_nothing_new() {
    let src = "ngon P 5 axis 1 rotation=2\ncayley D4 <r, s>\n";
    let (_, out) = round_trip(src);
    assert!(!out.contains('"'), "{out}");
}

#[test]
fn scene_draws_them_and_old_labels_are_untouched() {
    let (f, _) = parse("ngon P 4 axis 1 \"s\" rotation \"ρ\"\ncayley D4 <r, s> vertex r \"ρ\"\n");
    let sc = layout(&f);
    let labels: Vec<(&ObjRef, &str)> = sc.draws.iter().filter_map(|d| if let Draw::Label { obj, latex, .. } = d { Some((obj, latex.as_str())) } else { None }).collect();
    assert!(labels.iter().any(|(o, l)| **o == ObjRef::NGonAxis("P".into(), 2) && *l == "s"), "{labels:?}");
    assert!(labels.iter().any(|(o, l)| **o == ObjRef::NGonRotation("P".into()) && *l == r"\rho"));
    let cname = cayley(&f).name;
    assert!(labels.iter().any(|(o, l)| matches!(o, ObjRef::CayleyVertex(c, w) if *c == cname && w == "r") && *l == r"\rho"), "{labels:?}");
    // the other vertices keep their words
    assert!(labels.iter().any(|(o, l)| matches!(o, ObjRef::CayleyVertex(_, w) if w == "s") && *l == "s"));
    // default rotation label
    let (f, _) = parse("ngon P 5 rotation=2\n");
    let sc = layout(&f);
    assert!(sc.draws.iter().any(|d| matches!(d, Draw::Label { obj: ObjRef::NGonRotation(_), latex, .. } if latex == "r^{2}")));
}

#[test]
fn a_bare_quote_after_other_options_still_warns() {
    let (_, d) = parse("ngon P 4 \"x\"\n");
    assert!(!d.is_empty());
}
