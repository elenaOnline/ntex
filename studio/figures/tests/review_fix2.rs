//! Review fixes (worker 2): shorthand spelling kept on save (C11, U14, C2).
mod core_a_common;
use core_a_common::*;

fn round_trips(src: &str) -> String {
    let (f, _) = parse(src);
    let out = format(&f);
    let (g, _) = parse(&out);
    assert_eq!(without_spans(&f), without_spans(&g), "formatted:\n{out}");
    assert_eq!(format(&g), out, "second format differs");
    out
}

#[test]
fn negative_turn_keeps_her_spelling() {
    let out = round_trips("ngon P 5 turn=-9\n");
    assert_eq!(out.trim(), "ngon P 5 turn=-9");
    // the plain forms are untouched
    assert_eq!(round_trips("ngon P 5 turn=-1\n").trim(), "ngon P 5 turn=-1");
    assert_eq!(round_trips("ngon P 5 turn=7\n").trim(), "ngon P 5 turn=7");
    assert_eq!(round_trips("ngon P 5 turn=-5\n").trim(), "ngon P 5");
}

#[test]
fn explicit_default_blob_label_is_kept() {
    let out = round_trips("blob G \"G\" {a, b}\n");
    assert!(out.contains("blob G \"G\" {a, b}"), "{out}");
    let out = round_trips("blob G {a, b}\n");
    assert!(!out.contains("\"G\""), "{out}");
}

#[test]
fn option_order_is_kept_where_nothing_changed() {
    let out = round_trips("ngon P 4 at 6,-1 labels a,b,c,d\n");
    assert_eq!(out.trim(), "ngon P 4 at 6,-1 labels a,b,c,d");
    let out = round_trips("blob G {a} color=teal shaded at 2,2\n");
    assert!(out.contains("color=teal shaded at 2,2"), "{out}");
    let out = round_trips("cayley D4 <r, s> radius=2 legend at 1,1\n");
    assert!(out.contains("radius=2 legend at 1,1"), "{out}");
    // an option she adds goes last, the rest stays put
    let (mut f, _) = parse("ngon P 4 at 6,-1 labels a,b,c,d\n");
    if let Item::NGon(g) = &mut f.items[0] {
        g.rotation = 1;
    }
    let out = format(&f);
    assert!(out.starts_with("ngon P 4 at 6,-1 labels a,b,c,d"), "{out}");
    assert!(out.contains("rotation"), "{out}");
}

#[test]
fn cayley_vertex_words_round_trip_in_every_spelling() {
    for src in [
        "cayley D4 <ρ, σ> vertex ρσ \"x\"\n",
        "cayley D4 <ρ, σ> vertex ρ σ \"x\"\n",
        "cayley D4 <ρ, σ> vertex \"\\rho \\sigma\" \"x\"\n",
        "cayley D4 <r, s> vertex r^2s \"x\" vertex r² \"y\"\n",
    ] {
        let (f, d) = parse(src);
        no_errors(&d);
        assert!(warnings(&d).is_empty(), "{src}: {d:?}");
        let c = cayleys(&f).into_iter().next().unwrap();
        assert!(!c.vertex_names.is_empty(), "{src}");
        round_trips(src);
    }
    let (f, _) = parse("cayley D4 <r, s> vertex r^2s \"x\" vertex r² \"y\"\n");
    let c = cayleys(&f).into_iter().next().unwrap();
    assert_eq!(c.vertex_names, vec![("r^2s".to_string(), "x".to_string()), ("r^2".to_string(), "y".to_string())]);
}
