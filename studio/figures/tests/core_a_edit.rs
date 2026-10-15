//! FIG-CORE-A: canvas edits. Each test applies an edit, then checks that
//! `format` → `parse` gives the expected model (and the model itself).
mod core_a_common;
use core_a_common::*;

/// Apply, then check `parse(format(fig)) == fig` (spans aside); return the
/// result and the formatted source.
fn ed(fig: &mut Figure, edit: Edit) -> (Result<Option<String>, String>, String) {
    let r = apply(fig, &edit);
    let out = format(fig);
    let (g, _) = parse(&out);
    assert_eq!(without_spans(&g), without_spans(fig), "after {edit:?}\n--- formatted:\n{out}");
    assert_eq!(format(&g), out, "format not idempotent after {edit:?}");
    (r, out)
}

fn fig(src: &str) -> Figure {
    p(src).0
}

const SQUARE: &str = "% the square
grid
  A --f--> B
  |g       |h
  v        v
  C --k--> D
end
D -> A bend-left";

// ---------------------------------------------------------------- names

#[test]
fn fresh_names_run_a_to_z_then_a1() {
    let mut f = Figure::default();
    let mut got = Vec::new();
    for _ in 0..28 {
        let (r, _) = ed(&mut f, Edit::AddNode { label: String::new(), pos: Pos::Cell(0.0, 0.0) });
        got.push(r.unwrap().unwrap());
    }
    assert_eq!(&got[..3], ["A", "B", "C"]);
    assert_eq!(got[25], "Z");
    assert_eq!(&got[26..], ["A1", "B1"]);
}

#[test]
fn fresh_name_skips_every_kind_of_name() {
    let f = fig("A -> B\nblob C {D}\nngon E 3\ncayley F <a | a^2>");
    assert_eq!(fresh_name(&f), "G");
}

// ---------------------------------------------------------------- nodes

#[test]
fn add_node_appends_a_statement() {
    let mut f = fig("A -> B % keep me");
    let (r, out) = ed(&mut f, Edit::AddNode { label: "G".into(), pos: Pos::Cell(2.0, 0.0) });
    assert_eq!(r.unwrap().as_deref(), Some("G"));
    assert_eq!(out, "A -> B % keep me\nG at 2,0\n");
}

#[test]
fn add_node_with_a_latex_label_gets_a_fresh_name() {
    let mut f = fig("A -> B");
    let (r, out) = ed(&mut f, Edit::AddNode { label: "φ(G) ≅ H".into(), pos: Pos::Abs(1.0, -1.0) });
    assert_eq!(r.unwrap().as_deref(), Some("C"));
    assert!(out.ends_with("C \"φ(G) ≅ H\" at (1,-1)\n"), "{out}"); // her spelling
    assert_eq!(node(&f, "C").label, "\\varphi(G) \\cong H");
    // a label that is already a name also gets a fresh one
    let (r2, _) = ed(&mut f, Edit::AddNode { label: "A".into(), pos: Pos::Cell(0.0, 1.0) });
    assert_eq!(r2.unwrap().as_deref(), Some("D"));
    assert_eq!(node(&f, "D").label, "A");
}

#[test]
fn move_node_in_place() {
    let mut f = fig("A at 0,0\nB at 1,0\nA -> B");
    let (r, out) = ed(&mut f, Edit::MoveNode { name: "B".into(), pos: Pos::Cell(1.0, 1.0) });
    assert_eq!(r.unwrap().as_deref(), Some("B"));
    assert_eq!(out, "A at 0,0\nB at 1,1\nA -> B\n");
}

#[test]
fn move_an_implicit_node_writes_at_inline() {
    let mut f = fig("A -f-> B\nB -> C");
    let (_, out) = ed(&mut f, Edit::MoveNode { name: "B".into(), pos: Pos::Cell(1.0, 0.0) });
    assert_eq!(out, "A -f-> B at 1,0\nB -> C\n");
    assert_eq!(f.hints.implicit, ["A", "B", "C"]);
}

#[test]
fn move_a_grid_node_converts_the_block() {
    let mut f = fig(SQUARE);
    let (r, out) = ed(&mut f, Edit::MoveNode { name: "D".into(), pos: Pos::Cell(2.0, 1.0) });
    assert!(r.is_ok());
    assert!(f.grid_items.is_empty());
    assert!(!f.items.iter().any(|i| matches!(i, Item::Grid { .. })));
    assert!(out.starts_with("% the square\n% grid converted to statements (moved D)\nA at 0,0\nB at 1,0\nC at 0,1\nD at 2,1\nA -f-> B\n"), "{out}");
    assert!(out.ends_with("D -> A bend-left\n"));
    let (g, _) = parse(&out);
    assert_eq!(cell(&g, "D"), (2.0, 1.0));
    assert_eq!(cell(&g, "A"), (0.0, 0.0));
    assert_eq!(arrows(&g).len(), 5);
}

#[test]
fn move_other_objects() {
    let mut f = fig("blob G {g}\nngon P 4\ncayley D3 <r, s>");
    for (n, p) in [("g", Pos::Abs(0.1, 0.2)), ("P", Pos::Cell(3.0, 0.0)), ("D3", Pos::Abs(5.0, 0.0))] {
        let (r, _) = ed(&mut f, Edit::MoveNode { name: n.into(), pos: p });
        assert_eq!(r.unwrap().as_deref(), Some(n));
    }
    let (r, out) = ed(&mut f, Edit::MoveBlob { name: "G".into(), pos: Pos::Abs(-2.0, 0.0) });
    assert!(r.is_ok());
    assert_eq!(out, "blob G {g at (0.1,0.2)} at (-2,0)\nngon P 4 at 3,0\ncayley D3 <r, s> at (5,0)\n");
}

#[test]
fn set_node_label() {
    let mut f = fig("A -> B\nC at 1,1");
    ed(&mut f, Edit::SetNodeLabel { name: "C".into(), label: "ℤ/nℤ".into() }).0.unwrap();
    let (_, out) = ed(&mut f, Edit::SetNodeLabel { name: "A".into(), label: "say \"hi\"".into() });
    assert_eq!(out, "A \"say ''hi''\" -> B\nC \"ℤ/nℤ\" at 1,1\n"); // as she typed it
}

#[test]
fn set_label_on_a_grid_node_keeps_the_block() {
    // (FIG-INTERACT, 2026-10-12, by the coordinator's request) a relabel no
    // longer destroys her ASCII art: the block stays and the new label is a
    // statement after `end` (format's grid verdict). A move still converts.
    let mut f = fig(SQUARE);
    let (_, out) = ed(&mut f, Edit::SetNodeLabel { name: "A".into(), label: r"\mathbb{Z}".into() });
    assert!(out.contains("A --f--> B") && out.contains("\nend\n"), "{out}");
    assert!(out.contains("A \"\\mathbb{Z}\""), "{out}");
    assert!(!out.contains("converted"), "{out}");
}

#[test]
fn set_label_on_elements_and_blobs() {
    let mut f = fig("blob G {g}");
    ed(&mut f, Edit::SetNodeLabel { name: "g".into(), label: "γ".into() }).0.unwrap();
    let (_, out) = ed(&mut f, Edit::SetNodeLabel { name: "G".into(), label: "S_3".into() });
    assert_eq!(out, "blob G \"S_3\" {g \"γ\"}\n");
}

// ---------------------------------------------------------------- arrows

#[test]
fn add_arrow_appends() {
    let mut f = fig("A at 0,0\nB at 1,0");
    let (r, out) = ed(&mut f, Edit::AddArrow { from: "A".into(), to: "B".into(), label: Some("π".into()) });
    assert_eq!(r.unwrap().as_deref(), Some("2"));
    assert_eq!(out, "A at 0,0\nB at 1,0\nA -π-> B\n");
}

#[test]
fn add_arrow_to_every_kind_of_end() {
    let mut f = fig("blob G {g}\nblob H {h}\nngon P 4\ncayley D4 <r, s>\nX at 0,0");
    ed(&mut f, Edit::AddArrow { from: "G".into(), to: "H".into(), label: None }).0.unwrap();
    ed(&mut f, Edit::AddArrow { from: "g".into(), to: "h".into(), label: None }).0.unwrap();
    ed(&mut f, Edit::AddArrow { from: "P.2".into(), to: "X".into(), label: None }).0.unwrap();
    let (_, out) = ed(&mut f, Edit::AddArrow { from: "D4.rs".into(), to: "X".into(), label: None });
    assert!(out.ends_with("G -> H\ng -> h\nP.2 -> X\nD4.rs -> X\n"), "{out}");
}

#[test]
fn add_arrow_to_nothing_is_an_error() {
    let mut f = fig("A at 0,0\nngon P 4");
    let before = f.clone();
    assert!(apply(&mut f, &Edit::AddArrow { from: "A".into(), to: "Nope".into(), label: None }).is_err());
    assert!(apply(&mut f, &Edit::AddArrow { from: "A".into(), to: "P.7".into(), label: None }).is_err());
    assert_eq!(f, before);
}

#[test]
fn add_a_self_loop() {
    let mut f = fig("A at 0,0");
    ed(&mut f, Edit::AddArrow { from: "A".into(), to: "A".into(), label: None }).0.unwrap();
    assert!(arrows(&f)[0].is_loop);
}

#[test]
fn set_arrow_in_place() {
    let mut f = fig("A -> B\nB -> C % second");
    let mut a = arrows(&f)[1].clone();
    a.head = Head::TwoHeads;
    a.label = Some("π".into());
    a.bend = -30.0;
    let ix = f.items.iter().rposition(|i| matches!(i, Item::Arrow(_))).unwrap();
    let (r, out) = ed(&mut f, Edit::SetArrow { index: ix, arrow: a });
    assert_eq!(r.unwrap(), Some(ix.to_string()));
    assert_eq!(out, "A -> B\nB -π->> C bend-right % second\n");
}

#[test]
fn set_arrow_on_a_grid_arrow_converts_the_block() {
    let mut f = fig(SQUARE);
    let ix = f.items.iter().position(|i| matches!(i, Item::Arrow(a) if a.from == "A" && a.to == "B")).unwrap();
    let mut a = arrows(&f)[0].clone();
    a.line = Line::Dashed;
    let (_, out) = ed(&mut f, Edit::SetArrow { index: ix, arrow: a });
    assert!(out.contains("A --f--> B\n"), "{out}");
    assert!(out.contains("% grid converted to statements (changed an arrow)"));
}

#[test]
fn set_arrow_that_drops_an_implicit_end() {
    // B was named first by this arrow; re-pointing it makes B a statement
    let mut f = fig("A -> B\nC at 0,0");
    let mut a = arrows(&f)[0].clone();
    a.to = "C".into();
    let (_, out) = ed(&mut f, Edit::SetArrow { index: 2, arrow: a });
    assert_eq!(out, "A ; B ; A -> C\nC at 0,0\n");
}

// ---------------------------------------------------------------- blobs, n-gons, text

#[test]
fn add_blob_and_elements() {
    let mut f = fig("blob G {e, g}");
    let b = Blob {
        name: "K".into(),
        label: r"\ker\varphi".into(),
        elements: vec![],
        parent: Some("G".into()),
        shaded: true,
        color: None,
        shape: BlobShape::Round,
        size: Some((1.5, 1.0)),
        pos: None,
        label_place: LabelPlace::Above,
        span: Span::default(),
        angle: 0.0,
    };
    let (r, _) = ed(&mut f, Edit::AddBlob { blob: b });
    assert_eq!(r.unwrap().as_deref(), Some("K"));
    let (r, _) = ed(&mut f, Edit::AddElement { blob: "K".into(), label: "x".into(), pos: None });
    assert_eq!(r.unwrap().as_deref(), Some("x"));
    let (r, out) = ed(&mut f, Edit::AddElement { blob: "G".into(), label: "g^2".into(), pos: Some(Pos::Abs(0.5, 0.5)) });
    assert_eq!(r.unwrap().as_deref(), Some("g^2"));
    assert_eq!(out, "blob G {e, g, g^2 at (0.5,0.5)}\nblob K \"\\ker\\varphi\" {x} in G shaded size=1.5,1\n");
}

#[test]
fn add_blob_with_a_taken_name_is_renamed() {
    let mut f = fig("A -> B");
    let b = Blob {
        name: "A".into(),
        label: "A".into(),
        elements: vec![Element { name: "a".into(), label: "a".into(), pos: None }],
        parent: None,
        shaded: false,
        color: Some("teal".into()),
        shape: BlobShape::Tall,
        size: None,
        pos: Some(Pos::Abs(0.0, -3.0)),
        label_place: LabelPlace::Below,
        span: Span::default(),
        angle: 0.0,
    };
    let (r, out) = ed(&mut f, Edit::AddBlob { blob: b });
    assert_eq!(r.unwrap().as_deref(), Some("C"));
    assert_eq!(out, "A -> B\nblob C \"A\" {a} color=teal shape=tall at (0,-3) label=below\n");
}

#[test]
fn add_ngons() {
    let mut f = fig("P -> Q");
    let (r, _) = ed(&mut f, Edit::AddNGon { n: 5, pos: Pos::Cell(0.0, 2.0), radius: 1.0 });
    assert_eq!(r.unwrap().as_deref(), Some("R"));
    let (r, out) = ed(&mut f, Edit::AddNGon { n: 4, pos: Pos::Cell(1.0, 2.0), radius: 0.8 });
    assert_eq!(r.unwrap().as_deref(), Some("S"));
    assert!(out.ends_with("ngon R 5 radius=1 at 0,2\nngon S 4 radius=0.8 at 1,2\n"), "{out}");
    assert!(apply(&mut f, &Edit::AddNGon { n: 2, pos: Pos::Cell(0.0, 0.0), radius: 1.0 }).is_err());
}

#[test]
fn add_text() {
    let mut f = Figure::default();
    let (r, out) = ed(&mut f, Edit::AddText { text: "kernel ↦ e".into(), pos: Pos::Abs(1.0, 2.0) });
    assert_eq!(r.unwrap().as_deref(), Some("0"));
    assert_eq!(out, "text \"kernel ↦ e\" at (1,2)\n");
}

// ---------------------------------------------------------------- delete

#[test]
fn delete_a_node_removes_its_arrows() {
    let mut f = fig("A at 0,0\nB at 1,0\nC at 2,0\nA -> B\nB -> C\nA -> C");
    let (r, out) = ed(&mut f, Edit::Delete { obj: ObjRef::Node("B".into()) });
    assert_eq!(r.unwrap(), None);
    assert_eq!(out, "A at 0,0\nC at 2,0\nA -> C\n");
}

#[test]
fn delete_an_arrow_keeps_its_nodes() {
    let mut f = fig("A \"x\" -> B ; B -> C");
    let (_, out) = ed(&mut f, Edit::Delete { obj: ObjRef::Arrow(2) });
    // A was only named by that arrow: it becomes a statement
    assert_eq!(out, "A \"x\" ; B -> C\n");
    assert_eq!(nodes(&f).len(), 3);
}

#[test]
fn delete_a_grid_node_converts_the_block() {
    let mut f = fig(SQUARE);
    let (_, out) = ed(&mut f, Edit::Delete { obj: ObjRef::Node("C".into()) });
    assert!(out.contains("% grid converted to statements (deleted C)"));
    assert_eq!(edges(&f), [e("A", "B", Some("f")), e("B", "D", Some("h")), e("D", "A", None)]);
}

#[test]
fn delete_a_blob_takes_its_elements_and_arrows() {
    let mut f = fig("blob G \"G\" {e, g, g^2, g^3}\nblob K \"\\ker\\varphi\" in G shaded {e, g^2}\nblob L in K {z}\nblob H {h}\nG -> H\ng |-> h\ne |-> h\nz |-> h");
    let (_, out) = ed(&mut f, Edit::Delete { obj: ObjRef::Blob("K".into()) });
    // (review U14) her explicit `"G"` stays
    assert_eq!(out, "blob G \"G\" {g, g^3}\nblob L {z} in G\nblob H {h}\nG -> H\ng |-> h\nz |-> h\n");
    assert!(f.hints.listed.is_empty());
}

#[test]
fn delete_an_element_keeps_the_parents_list_tidy() {
    let mut f = fig("blob G {e, g, g^2}\nblob K in G {e, g^2}");
    let (_, out) = ed(&mut f, Edit::Delete { obj: ObjRef::Element("e".into()) });
    assert_eq!(out, "blob G {g, g^2}\nblob K {g^2} in G\n");
    let (_, out) = ed(&mut f, Edit::Delete { obj: ObjRef::Element("g^2".into()) });
    assert_eq!(out, "blob G {g}\nblob K in G\n");
    assert!(f.hints.listed.is_empty());
}

#[test]
fn delete_an_ngon_and_its_vertex_arrows() {
    let mut f = fig("ngon P 4\nngon Q 4\nP.1 -> Q.1\nP -> Q");
    let (_, out) = ed(&mut f, Edit::Delete { obj: ObjRef::NGon("P".into()) });
    assert_eq!(out, "ngon Q 4\n");
}

#[test]
fn delete_a_cayley_graph_through_a_vertex() {
    let mut f = fig("cayley D4 <r, s>\nX at 0,0\nD4.r -> X");
    let (_, out) = ed(&mut f, Edit::Delete { obj: ObjRef::CayleyVertex("D4".into(), "r".into()) });
    assert_eq!(out, "X at 0,0\n");
}

#[test]
fn delete_text_and_bad_refs() {
    let mut f = fig("text \"a\" at (0,0)\nA at 0,0");
    let (_, out) = ed(&mut f, Edit::Delete { obj: ObjRef::Text(0) });
    assert_eq!(out, "A at 0,0\n");
    let before = f.clone();
    assert!(apply(&mut f, &Edit::Delete { obj: ObjRef::Text(0) }).is_err());
    assert!(apply(&mut f, &Edit::Delete { obj: ObjRef::Node("Q".into()) }).is_err());
    assert!(apply(&mut f, &Edit::Delete { obj: ObjRef::Arrow(7) }).is_err());
    assert_eq!(f, before);
}

#[test]
fn errors_leave_the_figure_alone() {
    let mut f = fig(SQUARE);
    let before = f.clone();
    assert!(apply(&mut f, &Edit::MoveNode { name: "Nope".into(), pos: Pos::Cell(0.0, 0.0) }).is_err());
    assert!(apply(&mut f, &Edit::SetNodeLabel { name: "Nope".into(), label: "x".into() }).is_err());
    assert!(apply(&mut f, &Edit::MoveBlob { name: "A".into(), pos: Pos::Cell(0.0, 0.0) }).is_err());
    assert!(apply(&mut f, &Edit::AddElement { blob: "A".into(), label: "x".into(), pos: None }).is_err());
    assert!(apply(&mut f, &Edit::SetArrow { index: 0, arrow: arrows(&before)[0].clone() }).is_err());
    assert_eq!(f, before);
}

#[test]
fn untouched_lines_stay_as_written() {
    let src = "% header\nscale 1.2\n\nX -f-> Y   % first\nblob G {a, b}\n\n% tail";
    let mut f = fig(src);
    let (_, out) = ed(&mut f, Edit::AddNode { label: "Z".into(), pos: Pos::Cell(0.0, 1.0) });
    assert_eq!(out, "% header\nscale 1.2\n\nX -f-> Y % first\nblob G {a, b}\n\n% tail\nZ at 0,1\n");
}

// ---------------------------------------------------------------- property: random edits

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
    SQUARE,
    "blob G \"G\" {e, g, g^2, g^3}\nblob K \"\\ker\\varphi\" in G shaded {e, g^2}\nblob H \"H\" {e', h}\nG -φ-> H\ng |-> h ; e |-> e'",
    "ngon P 4\nngon Q 6 axis 1 axis 2.5 rotation=2\nP.1 -> Q.2\ncayley D4 <r, s>",
    "A -f-> B -g-> C\nB <-h- D % back\nA at 0,0",
    "grid\nX\n  \\u\n    P --> B\n    |     |g\n    v     v\n    A -f-> C\nend\nX -> A bend-right",
    "",
];

fn random_edit(r: &mut Rng, f: &Figure) -> Edit {
    let names: Vec<String> = nodes(f).iter().map(|n| n.name.clone()).collect();
    let mut ends = names.clone();
    for b in blobs(f) {
        ends.push(b.name.clone());
        ends.extend(b.elements.iter().map(|e| e.name.clone()));
    }
    for g in ngons(f) {
        ends.push(format!("{}.{}", g.name, 1 + r.below(g.n as usize)));
    }
    let pick = |r: &mut Rng, v: &Vec<String>| if v.is_empty() { "A".to_string() } else { v[r.below(v.len())].clone() };
    let arrow_ix: Vec<usize> = f.items.iter().enumerate().filter(|(_, i)| matches!(i, Item::Arrow(_))).map(|(k, _)| k).collect();
    let pos = |r: &mut Rng| if r.below(2) == 0 { Pos::Cell(r.below(4) as f32, r.below(3) as f32) } else { Pos::Abs(r.below(5) as f32 * 0.5, -(r.below(3) as f32)) };
    match r.below(14) {
        0 => Edit::AddNode { label: ["G", "", "φ(G)", "x y", "A", "\\mathbb{Z}"][r.below(6)].into(), pos: pos(r) },
        1 => Edit::MoveNode { name: pick(r, &names), pos: pos(r) },
        2 => Edit::SetNodeLabel { name: pick(r, &names), label: ["ℤ", "\\ker f", "", "q\"q"][r.below(4)].into() },
        3 | 4 => Edit::AddArrow { from: pick(r, &ends), to: pick(r, &ends), label: [None, Some("f".to_string()), Some("g ∘ f".to_string())][r.below(3)].clone() },
        5 => match arrow_ix.get(r.below(arrow_ix.len())) {
            Some(&i) => {
                let Item::Arrow(a) = &f.items[i] else { unreachable!() };
                let mut a = a.clone();
                a.head = [Head::To, Head::TwoHeads, Head::Hook, Head::Equal, Head::MapsTo, Head::Double, Head::Both, Head::Tail][r.below(8)];
                a.line = [Line::Solid, Line::Dashed, Line::Dotted][r.below(3)];
                a.bend = [0.0, 30.0, -30.0, 45.0][r.below(4)];
                a.to = pick(r, &ends);
                Edit::SetArrow { index: i, arrow: a }
            }
            None => Edit::AddText { text: "t".into(), pos: pos(r) },
        },
        6 => Edit::AddBlob {
            blob: Blob {
                name: ["N", "G", ""][r.below(3)].into(),
                label: "S".into(),
                elements: vec![],
                parent: blobs(f).first().map(|b| b.name.clone()).filter(|_| r.below(2) == 0),
                shaded: r.below(2) == 0,
                color: None,
                shape: BlobShape::Wide,
                size: None,
                pos: None,
                label_place: LabelPlace::Above,
                span: Span::default(),
                angle: 0.0,
            },
        },
        7 => Edit::AddElement { blob: blobs(f).first().map(|b| b.name.clone()).unwrap_or_default(), label: ["x", "g", "y'"][r.below(3)].into(), pos: None },
        8 => Edit::AddNGon { n: 3 + r.below(4) as u32, pos: pos(r), radius: 1.0 },
        9 => Edit::Delete { obj: ObjRef::Node(pick(r, &names)) },
        10 => match arrow_ix.get(r.below(arrow_ix.len())) {
            Some(&i) => Edit::Delete { obj: ObjRef::Arrow(i) },
            None => Edit::AddText { text: "u".into(), pos: pos(r) },
        },
        11 => match blobs(f).first() {
            Some(b) if r.below(2) == 0 => Edit::Delete { obj: ObjRef::Blob(b.name.clone()) },
            Some(b) if !b.elements.is_empty() => Edit::Delete { obj: ObjRef::Element(b.elements[0].name.clone()) },
            _ => Edit::AddText { text: "v".into(), pos: pos(r) },
        },
        12 => match ngons(f).first() {
            Some(g) => Edit::Delete { obj: ObjRef::NGon(g.name.clone()) },
            None => Edit::MoveBlob { name: "G".into(), pos: pos(r) },
        },
        _ => Edit::AddText { text: "kernel ↦ e".into(), pos: pos(r) },
    }
}

#[test]
fn property_random_edits_keep_the_round_trip() {
    let mut r = Rng(0xD1B54A32D192ED03);
    let mut applied = 0;
    for round in 0..300 {
        let mut f = fig(STARTS[round % STARTS.len()]);
        for _ in 0..12 {
            let e = random_edit(&mut r, &f);
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut g = f.clone();
                let r = apply(&mut g, &e);
                (g, r)
            }));
            let (g, res) = res.unwrap_or_else(|_| panic!("edit panicked: {e:?} on\n{}", format(&f)));
            if res.is_ok() {
                applied += 1;
                let out = format(&g);
                let (h, _) = parse(&out);
                assert_eq!(without_spans(&h), without_spans(&g), "after {e:?}\n--- before:\n{}\n--- after:\n{out}", format(&f));
                f = g;
            } else {
                assert_eq!(g, f, "a failed edit changed the figure: {e:?}");
            }
        }
    }
    assert!(applied > 2000, "only {applied} edits applied");
}
