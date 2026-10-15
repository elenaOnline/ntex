//! FIG-CORE-A: the parser. Every case also checks the format round trip.
mod core_a_common;
use core_a_common::*;

// ---------------------------------------------------------------- basics

#[test]
fn empty_source() {
    let (f, d) = p("");
    assert!(f.items.is_empty());
    assert!(d.is_empty());
}

#[test]
fn single_arrow_makes_two_nodes() {
    let (f, d) = p("A -> B");
    assert_eq!(nodes(&f).iter().map(|n| n.name.as_str()).collect::<Vec<_>>(), ["A", "B"]);
    assert_eq!(edges(&f), [e("A", "B", None)]);
    assert_eq!(f.hints.implicit, ["A", "B"]);
    // auto-created nodes are notes, not warnings
    assert!(d.iter().all(|x| x.severity == Severity::Note));
    assert_eq!(d.len(), 2);
}

#[test]
fn node_with_quoted_label() {
    let (f, d) = p(r#"A "\mathbb{Z}/n\mathbb{Z}""#);
    no_errors(&d);
    assert_eq!(node(&f, "A").label, r"\mathbb{Z}/n\mathbb{Z}");
    assert!(f.hints.implicit.is_empty());
}

#[test]
fn token_is_its_own_label() {
    let (f, _) = p(r"\ker\varphi -> G/K");
    assert_eq!(node(&f, r"\ker\varphi").label, r"\ker\varphi");
    assert_eq!(node(&f, "G/K").label, "G/K");
}

#[test]
fn balanced_braces_inside_tokens() {
    let (f, d) = p(r"\mathbb{Z} -> G^{ab}");
    no_errors(&d);
    assert_eq!(edges(&f), [e(r"\mathbb{Z}", "G^{ab}", None)]);
}

#[test]
fn primes_and_subscripts_in_names() {
    let (f, _) = p("H' -> Z_n");
    assert_eq!(edges(&f), [e("H'", "Z_n", None)]);
}

#[test]
fn first_mention_wins_for_label() {
    let (f, d) = p("A \"x\" -> B\nA \"y\" -> C");
    assert_eq!(node(&f, "A").label, "x");
    assert_eq!(warnings(&d).len(), 1);
    assert!(warnings(&d)[0].message.starts_with("line 2:"));
}

#[test]
fn later_label_fills_a_default() {
    // a bare first mention has no label of its own: a later quoted one is used
    let (f, d) = p("A -> B\nA \"\\mathbb{Z}\"");
    no_errors(&d);
    assert_eq!(node(&f, "A").label, r"\mathbb{Z}");
    // A now has its own statement; B is still implicit
    assert_eq!(f.hints.implicit, ["B"]);
}

#[test]
fn later_mentions_just_refer() {
    let (f, _) = p("A -> B\nB -> C\nC -> A");
    assert_eq!(nodes(&f).len(), 3);
    assert_eq!(arrows(&f).len(), 3);
}

#[test]
fn name_used_before_its_statement() {
    let (f, d) = p("A -> B\nA at 1,0");
    no_errors(&d);
    // the node item sits where it is declared
    assert!(matches!(&f.items[2], Item::Node(n) if n.name == "A" && n.pos == Some(Pos::Cell(1.0, 0.0))));
    assert_eq!(f.hints.implicit, ["B"]);
}

#[test]
fn chain_of_arrows() {
    let (f, _) = p("A -f-> B -g-> C dashed");
    assert_eq!(edges(&f), [e("A", "B", Some("f")), e("B", "C", Some("g"))]);
    assert!(arrows(&f).iter().all(|a| a.line == Line::Dashed));
}

#[test]
fn compact_arrows_without_spaces() {
    let (f, d) = p("A->B ; C->>D ; E|->F");
    no_errors(&d);
    let hs: Vec<Head> = arrows(&f).iter().map(|a| a.head).collect();
    assert_eq!(hs, [Head::To, Head::TwoHeads, Head::MapsTo]);
    assert_eq!(edges(&f)[0], e("A", "B", None));
}

// ---------------------------------------------------------------- arrows

fn one(src: &str) -> Arrow {
    let (f, d) = p(src);
    no_errors(&d);
    let a = arrows(&f);
    assert_eq!(a.len(), 1, "{src}: {a:?}");
    a[0].clone()
}

#[test]
fn every_arrow_form_in_the_spec_table() {
    // read the table from FIGURES.md so the spec and the parser can't drift
    let spec = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/FIGURES.md")).unwrap();
    let mut rows = 0;
    let table = spec.split("| Write | Means").nth(1).unwrap().split("\n\n").next().unwrap();
    for line in table.lines().filter(|l| l.starts_with("| `")) {
        let cells: Vec<&str> = line.split(" | ").collect();
        let write = cells[0].trim_start_matches("| ");
        let means = cells[1];
        let want = if means.contains("surjection") {
            Head::TwoHeads
        } else if means.contains("injection") {
            Head::Tail
        } else if means.contains("inclusion") {
            Head::Hook
        } else if means.contains("double") && !means.contains("no head") {
            Head::Double
        } else if means.contains("both") {
            Head::Both
        } else if means.contains("equals") {
            Head::Equal
        } else if means.contains("maps-to") {
            Head::MapsTo
        } else {
            Head::To
        };
        let line_want = if means.contains("dashed") {
            Line::Dashed
        } else if means.contains("dotted") {
            Line::Dotted
        } else {
            Line::Solid
        };
        // the cell holds backticked examples
        for ex in write.split('`').skip(1).step_by(2) {
            let ex = ex.replace(r"\|", "|");
            let a = one(&ex);
            assert_eq!(a.head, want, "{ex} ({means})");
            assert_eq!(a.line, line_want, "{ex} ({means})");
            if means.contains("reversed") {
                assert_eq!((a.from.as_str(), a.to.as_str()), ("A", "B"), "{ex}");
            }
            rows += 1;
        }
    }
    assert!(rows >= 20, "only {rows} examples read from the table");
}

#[test]
fn labelled_arrow() {
    let a = one("A -f-> B");
    assert_eq!(a.label.as_deref(), Some("f"));
    assert_eq!((a.head, a.line), (Head::To, Line::Solid));
}

#[test]
fn quoted_label_with_spaces() {
    let a = one(r#"A -"g \circ f"-> B"#);
    assert_eq!(a.label.as_deref(), Some(r"g \circ f"));
}

#[test]
fn label_with_inner_dash_needs_no_quotes() {
    let a = one("A -g-1-> B");
    assert_eq!(a.label.as_deref(), Some("g-1"));
}

#[test]
fn surjection_with_label() {
    let a = one("A -π->> B");
    assert_eq!(a.head, Head::TwoHeads);
    assert_eq!(a.label.as_deref(), Some(r"\pi"));
}

#[test]
fn injection_and_inclusion() {
    assert_eq!(one("A >-ι-> B").head, Head::Tail);
    let h = one("A -ι-hook-> B");
    assert_eq!(h.head, Head::Hook);
    assert_eq!(h.label.as_deref(), Some(r"\iota"));
    assert_eq!(one("A hook-> B").head, Head::Hook);
}

#[test]
fn dashed_and_dotted() {
    let a = one("A --∃!--> B");
    assert_eq!((a.line, a.label.as_deref()), (Line::Dashed, Some(r"\exists!")));
    assert_eq!(one("A .f.> B").line, Line::Dotted);
    assert_eq!(one("A ..> B").line, Line::Dotted);
}

#[test]
fn reversed_arrow_swaps_ends() {
    let a = one("B <-f- A");
    assert_eq!((a.from.as_str(), a.to.as_str(), a.label.as_deref()), ("A", "B", Some("f")));
}

#[test]
fn both_ways_and_equals() {
    assert_eq!(one("A <-f-> B").head, Head::Both);
    assert_eq!(one("A = B").head, Head::Equal);
    assert_eq!(one("A =f=> B").head, Head::Double);
}

#[test]
fn maps_to_with_unicode_arrow() {
    let a = one("g ↦ h");
    assert_eq!(a.head, Head::MapsTo);
    assert_eq!(one("A → B").head, Head::To);
}

#[test]
fn all_arrow_options() {
    let a = one("A -f-> B bend=40 swap iso thick color=teal near-end dotted");
    assert_eq!(a.bend, 40.0);
    assert_eq!(a.side, LabelSide::Swap);
    assert!(a.iso && a.thick);
    assert_eq!(a.color.as_deref(), Some("teal"));
    assert_eq!(a.label_pos, LabelPos::NearEnd);
    assert_eq!(a.line, Line::Dotted);
}

#[test]
fn bend_left_and_right() {
    assert_eq!(one("A -> B bend").bend, 30.0);
    assert_eq!(one("A -> B bend-left").bend, 30.0);
    assert_eq!(one("A -> B bend-right").bend, -30.0);
    assert_eq!(one("A -> B bend-right=50").bend, -50.0);
    assert_eq!(one("A -> B near-start").label_pos, LabelPos::NearStart);
}

#[test]
fn self_loop() {
    let (f, d) = p("A -f-> A loop");
    no_errors(&d);
    assert_eq!(nodes(&f).len(), 1);
    assert!(arrows(&f)[0].is_loop);
    // `loop` is implied by the same node at both ends
    let (g, _) = p("A -> A");
    assert!(arrows(&g)[0].is_loop);
}

#[test]
fn unknown_option_is_a_warning() {
    let (f, d) = p("A -> B wiggly");
    assert_eq!(arrows(&f).len(), 1);
    assert_eq!(warnings(&d).len(), 1);
    assert!(errors(&d).is_empty());
    assert!(warnings(&d)[0].message.contains("wiggly"));
}

#[test]
fn unknown_colour_is_dropped_with_a_warning() {
    let (f, d) = p("A -> B color=fuchsia");
    assert_eq!(arrows(&f)[0].color, None);
    assert_eq!(warnings(&d).len(), 1);
    assert_eq!(one("A -> B color=gray").color.as_deref(), Some("grey"));
}

// ---------------------------------------------------------------- placement

#[test]
fn cells_and_absolute_positions() {
    let (f, d) = p("A at 1,0\nB at 0.5, 1\nC at (3.2,-1)\nD at ( -1 , 2.5 )\nE at 2 3");
    no_errors(&d);
    assert_eq!(node(&f, "A").pos, Some(Pos::Cell(1.0, 0.0)));
    assert_eq!(node(&f, "B").pos, Some(Pos::Cell(0.5, 1.0)));
    assert_eq!(node(&f, "C").pos, Some(Pos::Abs(3.2, -1.0)));
    assert_eq!(node(&f, "D").pos, Some(Pos::Abs(-1.0, 2.5)));
    assert_eq!(node(&f, "E").pos, Some(Pos::Cell(2.0, 3.0)));
}

#[test]
fn inline_at_inside_an_arrow() {
    let (f, d) = p("A at 0,0 -> B \"\\mathbb{Z}\" at 1,0");
    no_errors(&d);
    assert_eq!(node(&f, "A").pos, Some(Pos::Cell(0.0, 0.0)));
    assert_eq!(node(&f, "B").pos, Some(Pos::Cell(1.0, 0.0)));
    assert_eq!(node(&f, "B").label, r"\mathbb{Z}");
}

#[test]
fn node_colour() {
    let (f, d) = p("A at 0,0 color=maroon");
    no_errors(&d);
    assert_eq!(node(&f, "A").color.as_deref(), Some("maroon"));
}

#[test]
fn bad_position_is_an_error_and_kept() {
    let (f, d) = p("A at here\nB -> C");
    assert_eq!(errors(&d).len(), 1);
    assert!(errors(&d)[0].message.starts_with("line 1:"));
    assert!(matches!(&f.items[0], Item::Comment { text, .. } if text == "A at here"));
    assert_eq!(arrows(&f).len(), 1);
}

#[test]
fn globals() {
    let (f, d) = p("scale 1.2\nspacing 2.8,2\nsize small\nA -> B");
    no_errors(&d);
    assert_eq!(f.globals.scale, 1.2);
    assert_eq!(f.globals.spacing, (2.8, 2.0));
    assert_eq!(f.globals.size, "small");
    let (g, _) = p("spacing 3, 2.5\nsize=large");
    assert_eq!(g.globals.spacing, (3.0, 2.5));
    assert_eq!(g.globals.size, "large");
}

#[test]
fn bad_global_is_an_error() {
    let (f, d) = p("scale big\nsize huge");
    assert_eq!(errors(&d).len(), 2);
    assert_eq!(f.globals, Globals::default());
}

// ---------------------------------------------------------------- separators and comments

#[test]
fn semicolons_separate_statements() {
    let (f, d) = p("g |-> h ; e |-> e' ; A");
    no_errors(&d);
    assert_eq!(arrows(&f).len(), 2);
    assert!(nodes(&f).iter().any(|n| n.name == "A"));
}

#[test]
fn escaped_semicolon_and_percent_stay_in_labels() {
    let (f, d) = p(r#"A -"a\;b"-> B ; C "50\%" % real comment"#);
    no_errors(&d);
    assert_eq!(arrows(&f)[0].label.as_deref(), Some(r"a\;b"));
    assert_eq!(node(&f, "C").label, r"50\%");
    assert!(matches!(f.items.last(), Some(Item::Comment { text, .. }) if text == "% real comment"));
}

#[test]
fn comments_and_blank_lines_are_kept() {
    let (f, _) = p("% the kernel\n\nA -> B  % first map\n\n% end");
    let comments: Vec<&str> = f.items.iter().filter_map(|i| if let Item::Comment { text, .. } = i { Some(text.as_str()) } else { None }).collect();
    assert_eq!(comments, ["% the kernel", "", "% first map", "", "% end"]);
}

#[test]
fn percent_inside_quotes_is_not_a_comment() {
    let (f, _) = p(r#"A "50%" -> B"#);
    assert_eq!(node(&f, "A").label, "50%");
}

#[test]
fn crlf_and_tabs() {
    let (f, d) = p("A\t->\tB\r\nC -> D\r\n");
    no_errors(&d);
    assert_eq!(arrows(&f).len(), 2);
}

// ---------------------------------------------------------------- unicode

#[test]
fn unicode_labels() {
    let (f, _) = p("φ(G) -> ℤ/nℤ");
    assert_eq!(node(&f, "φ(G)").label, r"\varphi(G)");
    assert_eq!(node(&f, "ℤ/nℤ").label, r"\mathbb{Z}/n\mathbb{Z}");
}

#[test]
fn unicode_and_latex_name_the_same_node() {
    let (f, _) = p("φ -> A\n\\varphi -> B");
    assert_eq!(nodes(&f).len(), 3);
    assert_eq!(arrows(&f)[1].from, "φ");
}

#[test]
fn unicode_scripts_and_operators() {
    let (f, _) = p("x₁ -> g² ; A×B -> A⊗B ; G -≅-> H");
    assert_eq!(node(&f, "x₁").label, "x_{1}");
    assert_eq!(node(&f, "g²").label, "g^{2}");
    assert_eq!(node(&f, "A×B").label, r"A\times B");
    assert_eq!(node(&f, "A⊗B").label, r"A\otimes B");
    assert_eq!(arrows(&f)[2].label.as_deref(), Some(r"\cong"));
}

#[test]
fn unicode_text_becomes_maths_islands() {
    let (f, _) = p("text \"kernel ↦ identity\" at (1,2)");
    match &f.items[0] {
        Item::Text(t) => {
            assert_eq!(t.text, r"kernel $\mapsto$ identity");
            assert_eq!(t.pos, Pos::Abs(1.0, 2.0));
        }
        other => panic!("{other:?}"),
    }
}

// ---------------------------------------------------------------- blobs

const BLOBS: &str = "blob G \"G\" {e, g, g^2, g^3}
blob K \"\\ker\\varphi\" in G shaded {e, g^2}
blob H \"H\" {e', h}
G -φ-> H
g |-> h ; e |-> e'";

#[test]
fn blob_example_from_the_spec() {
    let (f, d) = p(BLOBS);
    no_errors(&d);
    let bs = blobs(&f);
    assert_eq!(bs.len(), 3);
    // elements listed again in the child live only in the child
    let names = |b: &Blob| b.elements.iter().map(|e| e.name.clone()).collect::<Vec<_>>();
    assert_eq!(names(bs[0]), ["g", "g^3"]);
    assert_eq!(names(bs[1]), ["e", "g^2"]);
    assert_eq!(bs[1].parent.as_deref(), Some("G"));
    assert!(bs[1].shaded);
    assert_eq!(bs[1].label, r"\ker\varphi");
    assert_eq!(edges(&f), [e("G", "H", Some(r"\varphi")), e("g", "h", None), e("e", "e'", None)]);
    assert_eq!(arrows(&f)[1].head, Head::MapsTo);
    // no stray nodes: everything resolved to blobs and elements
    assert!(nodes(&f).is_empty());
    // the parent's list is remembered as written
    assert_eq!(f.hints.listed, [("G".to_string(), vec!["e".to_string(), "g".into(), "g^2".into(), "g^3".into()])]);
    assert!(format(&f).contains("blob G {e, g, g^2, g^3}"));
}

#[test]
fn blob_options() {
    let (f, d) = p("blob A \"A\" {x \"\\bar x\", y at (0.5,0.2)} color=violet shape=bean size=3,2 at (1,1) label=inside");
    no_errors(&d);
    let b = blobs(&f)[0];
    assert_eq!(b.color.as_deref(), Some("violet"));
    assert_eq!(b.shape, BlobShape::Bean);
    assert_eq!(b.size, Some((3.0, 2.0)));
    assert_eq!(b.pos, Some(Pos::Abs(1.0, 1.0)));
    assert_eq!(b.label_place, LabelPlace::Inside);
    assert_eq!(b.elements[0].label, r"\bar x");
    assert_eq!(b.elements[1].pos, Some(Pos::Abs(0.5, 0.2)));
}

#[test]
fn element_cell_position_inside_braces() {
    let (f, d) = p("blob A {x at 1,0, y}");
    no_errors(&d);
    let b = blobs(&f)[0];
    assert_eq!(b.elements.len(), 2);
    assert_eq!(b.elements[0].pos, Some(Pos::Cell(1.0, 0.0)));
}

#[test]
fn blob_without_label_uses_its_name() {
    let (f, _) = p("blob G {a}");
    assert_eq!(blobs(&f)[0].label, "G");
    let (g, _) = p("blob K \"\" {a}");
    assert_eq!(blobs(&g)[0].label, "");
}

#[test]
fn element_used_before_its_blob() {
    let (f, d) = p("a |-> b\nblob A {a}\nblob B {b}");
    no_errors(&d);
    assert!(nodes(&f).is_empty());
    assert_eq!(edges(&f), [e("a", "b", None)]);
}

#[test]
fn duplicate_element_in_unrelated_blobs_is_an_error() {
    let (f, d) = p("blob A {x}\nblob B {x, y}");
    assert_eq!(errors(&d).len(), 1);
    assert_eq!(blobs(&f)[1].elements.len(), 1);
}

#[test]
fn missing_parent_is_a_warning() {
    let (f, d) = p("blob K in G {e}");
    assert_eq!(warnings(&d).len(), 1);
    assert_eq!(blobs(&f)[0].parent, None);
}

#[test]
fn parent_declared_later_is_fine() {
    let (f, d) = p("blob K in G {e}\nblob G {e, g}");
    no_errors(&d);
    assert_eq!(blobs(&f)[0].parent.as_deref(), Some("G"));
    assert_eq!(blobs(&f)[0].elements.len(), 1);
    assert_eq!(blobs(&f)[1].elements.len(), 1);
}

#[test]
fn duplicate_blob_is_kept_as_written() {
    let (f, d) = p("blob G {a}\nblob G {b}");
    assert_eq!(errors(&d).len(), 1);
    assert_eq!(blobs(&f).len(), 1);
    assert!(matches!(&f.items[1], Item::Comment { text, .. } if text == "blob G {b}"));
}

// ---------------------------------------------------------------- n-gons

#[test]
fn ngon_examples_from_the_spec() {
    let (f, d) = p("ngon P 4\nngon P5 5 labels a,b,c,d,e axes rotation\nngon Q 6 axis 1 axis 2.5 rotation=2");
    no_errors(&d);
    let g = ngons(&f);
    assert_eq!((g[0].n, g[0].labels.clone()), (4, vec!["1".into(), "2".into(), "3".into(), "4".into()]));
    assert!(g[1].all_axes);
    assert_eq!(g[1].rotation, 1);
    assert_eq!(g[1].labels, ["a", "b", "c", "d", "e"]);
    // vertex k → 2k, edge k.5 → 2k+1
    assert_eq!(g[2].axes, [2, 5]);
    assert_eq!(g[2].rotation, 2);
}

#[test]
fn ngon_turn_flip_radius_at() {
    let (f, d) = p("ngon P 4 turn=1 flip=2.5 radius=1.5 at (2,0) labels a, b, c, d");
    no_errors(&d);
    let g = ngons(&f)[0];
    assert_eq!((g.turn, g.flip, g.radius, g.pos), (1, Some(5), Some(1.5), Some(Pos::Abs(2.0, 0.0))));
    assert_eq!(g.labels, ["a", "b", "c", "d"]);
}

#[test]
fn odd_ngon_edge_axis_is_the_opposite_vertex() {
    let (f, _) = p("ngon T 3 axis 1.5");
    // edge 1 (between 1 and 2) of a triangle faces vertex 3
    assert_eq!(ngons(&f)[0].axes, [6]);
}

#[test]
fn ngon_label_count_mismatch_is_padded() {
    let (f, d) = p("ngon P 4 labels a,b");
    assert_eq!(warnings(&d).len(), 1);
    assert_eq!(ngons(&f)[0].labels, ["a", "b", "3", "4"]);
}

#[test]
fn ngon_without_a_name_gets_one() {
    let (f, d) = p("ngon 6");
    assert!(errors(&d).is_empty());
    assert_eq!(ngons(&f)[0].name, "P");
    assert_eq!(ngons(&f)[0].n, 6);
}

#[test]
fn ngon_vertex_references() {
    let (f, d) = p("ngon P 4 labels a,b,c,d\nngon Q 4\nP.1 -> Q.3 ; P.c -> Q.1");
    no_errors(&d);
    assert_eq!(edges(&f), [e("P.1", "Q.3", None), e("P.3", "Q.1", None)]);
    assert!(nodes(&f).is_empty());
}

#[test]
fn bad_ngon_vertex_is_an_error() {
    let (f, d) = p("ngon P 4\nP.9 -> P.1");
    assert_eq!(errors(&d).len(), 1);
    assert!(arrows(&f).is_empty());
}

#[test]
fn ngon_with_too_few_sides() {
    let (f, d) = p("ngon P 2\nA -> B");
    assert_eq!(errors(&d).len(), 1);
    assert!(ngons(&f).is_empty());
    assert_eq!(arrows(&f).len(), 1);
}

// ---------------------------------------------------------------- cayley

#[test]
fn cayley_examples_from_the_spec() {
    let (f, d) = p("cayley D4 <r, s>\ncayley <a, b | a^4, b^2, (ab)^2>\ncayley Z6 <1>");
    no_errors(&d);
    let c = cayleys(&f);
    assert_eq!(c[0].group, GroupSpec::Named("D4".into()));
    assert_eq!(c[0].gens, ["r", "s"]);
    assert_eq!(c[0].name, "D4");
    assert_eq!(c[1].group, GroupSpec::Presentation(vec!["a".into(), "b".into()], vec!["a^4".into(), "b^2".into(), "(ab)^2".into()]));
    assert_eq!(c[1].name, "C");
    assert_eq!(c[2].gens, ["1"]);
}

#[test]
fn cayley_group_names_are_normalised() {
    let (f, d) = p("cayley z2 × ℤ3 <a, b>\ncayley V4 <a, b> layout=grid legend labels=none radius=2 name=K");
    no_errors(&d);
    let c = cayleys(&f);
    assert_eq!(c[0].group, GroupSpec::Named("Z2xZ3".into()));
    assert_eq!(c[1].name, "K");
    assert_eq!(c[1].layout, CayleyLayout::Grid);
    assert!(c[1].legend && !c[1].labels);
    assert_eq!(c[1].radius, Some(2.0));
}

#[test]
fn cayley_named_presentation_and_unicode_relators() {
    let (f, d) = p("cayley D3 <r, s | r³, s², (rs)²>");
    no_errors(&d);
    let c = cayleys(&f)[0];
    assert_eq!(c.name, "D3");
    assert_eq!(c.group, GroupSpec::Presentation(vec!["r".into(), "s".into()], vec!["r^3".into(), "s^2".into(), "(rs)^2".into()]));
}

#[test]
fn cayley_vertex_reference() {
    let (f, d) = p("cayley D4 <r, s>\nD4.e -> D4.rs dashed");
    no_errors(&d);
    assert_eq!(edges(&f), [e("D4.e", "D4.rs", None)]);
}

#[test]
fn unknown_group_is_an_error() {
    let (f, d) = p("cayley Monster <a>");
    assert_eq!(errors(&d).len(), 1);
    assert!(cayleys(&f).is_empty());
}

// ---------------------------------------------------------------- text

#[test]
fn text_without_quotes_or_position() {
    let (f, d) = p("text hello world");
    assert_eq!(warnings(&d).len(), 1);
    assert!(matches!(&f.items[0], Item::Text(t) if t.text == "hello world"));
}

// ---------------------------------------------------------------- grids

const SQUARE: &str = "grid
  A --f--> B
  |g       |h
  v        v
  C --k--> D
end";

#[test]
fn grid_square() {
    let (f, d) = p(SQUARE);
    no_errors(&d);
    assert_eq!(cell(&f, "A"), (0.0, 0.0));
    assert_eq!(cell(&f, "B"), (1.0, 0.0));
    assert_eq!(cell(&f, "C"), (0.0, 1.0));
    assert_eq!(cell(&f, "D"), (1.0, 1.0));
    assert_eq!(edges(&f), [e("A", "B", Some("f")), e("A", "C", Some("g")), e("B", "D", Some("h")), e("C", "D", Some("k"))]);
    assert!(arrows(&f).iter().all(|a| a.line == Line::Solid && a.head == Head::To));
    // the block, then its nodes and arrows, all recorded
    assert!(matches!(&f.items[0], Item::Grid { .. }));
    assert_eq!(f.grid_items.len(), 8);
    assert!(f.grid_items.iter().all(|&(_, g)| g == 0));
    assert_eq!(format(&f), format!("{SQUARE}\n"));
}

#[test]
fn grid_triangle_with_diagonal() {
    let (f, d) = p("grid
A --f--> B
  \\h     |g
         v
         C
end");
    no_errors(&d);
    assert_eq!(cell(&f, "C"), (1.0, 1.0));
    assert_eq!(edges(&f), [e("A", "B", Some("f")), e("B", "C", Some("g")), e("A", "C", Some("h"))]);
}

#[test]
fn grid_pullback_with_corner_arrow() {
    let src = "grid
X
  \\u
    P ----> B
    |       |g
    v       v
    A --f-> C
end
X -> A bend-right
X -> B bend-left";
    let (f, d) = p(src);
    no_errors(&d);
    assert_eq!(cell(&f, "X"), (0.0, 0.0));
    assert_eq!(cell(&f, "P"), (1.0, 1.0));
    assert_eq!(cell(&f, "B"), (2.0, 1.0));
    assert_eq!(cell(&f, "A"), (1.0, 2.0));
    let es = edges(&f);
    assert!(es.contains(&e("X", "P", Some("u"))));
    assert!(es.contains(&e("P", "B", None)));
    assert!(es.contains(&e("P", "A", None)));
    assert!(es.contains(&e("B", "C", Some("g"))));
    assert!(es.contains(&e("A", "C", Some("f"))));
    // statements after `end` add arrows between the grid's nodes
    assert_eq!(arrows(&f).iter().filter(|a| a.bend != 0.0).count(), 2);
    assert_eq!(nodes(&f).len(), 5);
}

#[test]
fn grid_mixed_labels() {
    let src = "grid
G --φ--> H --\"g \\circ f\"--> K
g|       |\\iota
v        v
G' -\\pi-> H'
end";
    let (f, d) = p(src);
    no_errors(&d);
    let es = edges(&f);
    assert_eq!(es[0], e("G", "H", Some(r"\varphi")));
    assert_eq!(es[1], e("H", "K", Some(r"g \circ f")));
    assert!(es.contains(&e("G", "G'", Some("g"))));
    assert!(es.contains(&e("H", "H'", Some(r"\iota"))));
    assert!(es.contains(&e("G'", "H'", Some(r"\pi"))));
    // a label written left of the pipe goes on the other side
    let left = arrows(&f).into_iter().find(|a| a.to == "G'").unwrap();
    assert_eq!(left.side, LabelSide::Swap);
    assert!(arrows(&f).iter().filter(|a| a.to != "G'").all(|a| a.side == LabelSide::Auto));
}

#[test]
fn grid_up_arrows() {
    let (f, d) = p("grid
A --f--> B
^        ^
|g       |h
C --k--> D
end");
    no_errors(&d);
    let es = edges(&f);
    assert!(es.contains(&e("C", "A", Some("g"))));
    assert!(es.contains(&e("D", "B", Some("h"))));
}

#[test]
fn grid_dashed_verticals_equals_and_spaced_dashes() {
    let (f, d) = p("grid
A - - > B
:       ‖
v       ‖
C ..>   D
end");
    no_errors(&d);
    let a = arrows(&f);
    let find = |x: &str, y: &str| a.iter().find(|q| q.from == x && q.to == y).unwrap_or_else(|| panic!("{x}->{y} in {a:#?}"));
    assert_eq!(find("A", "B").line, Line::Dashed);
    assert_eq!(find("A", "C").line, Line::Dashed);
    assert_eq!(find("B", "D").head, Head::Equal);
    assert_eq!(find("C", "D").line, Line::Dotted);
}

#[test]
fn grid_misaligned_by_one() {
    let (f, d) = p("grid
 G --φ--> H
 |        |
 v        v
G/K -----> H/φ(K)
end");
    no_errors(&d);
    assert_eq!(cell(&f, "G"), (0.0, 0.0));
    assert_eq!(cell(&f, "G/K"), (0.0, 1.0));
    assert_eq!(cell(&f, "H"), (1.0, 0.0));
    assert_eq!(cell(&f, "H/φ(K)"), (1.0, 1.0));
    assert_eq!(arrows(&f).len(), 4);
}

#[test]
fn grid_bastardized_quick_shorthand() {
    // a Cayley-ish sketch: statement-style arrows, pipes with no `v`
    // (they point down), a reversed bottom row, a two-headed map.
    let (f, d) = p("grid
e -r-> r -r-> r^2
|s     |s     |s
s <-r- rs <-r- r^2s
r^2 ->> e
end");
    assert!(errors(&d).is_empty() && warnings(&d).is_empty());
    let es = edges(&f);
    assert!(es.contains(&e("e", "r", Some("r"))));
    assert!(es.contains(&e("r", "r^2", Some("r"))));
    assert!(es.contains(&e("e", "s", Some("s"))));
    assert!(es.contains(&e("r^2", "r^2s", Some("s"))));
    assert!(es.contains(&e("rs", "s", Some("r"))));
    assert!(es.contains(&e("r^2s", "rs", Some("r"))));
    // `r^2` and `e` written again are new nodes with the same labels
    assert!(es.contains(&e("r^2#2", "e#2", None)));
    assert_eq!(nodes(&f).len(), 8);
    assert_eq!(node(&f, "e#2").label, "e");
    // the pipes without heads were noted
    assert!(d.iter().any(|x| x.severity == Severity::Note && x.message.contains("points down")));
}

#[test]
fn grid_without_end() {
    let (f, d) = p("grid\nA --> B\n|     |\nv     v\nC --> D\nblob X {x}");
    assert_eq!(errors(&d).len(), 1);
    assert_eq!(arrows(&f).len(), 4);
    assert_eq!(blobs(&f).len(), 1);
    // the formatter closes the block
    assert!(format(&f).contains("C --> D\nend\nblob X"));
}

#[test]
fn grid_long_tokens_and_command_rows() {
    let (f, d) = p("grid
\\ker\\varphi --> G --φ->> \\mathrm{im}\\varphi
end");
    no_errors(&d);
    assert_eq!(cell(&f, r"\mathrm{im}\varphi"), (2.0, 0.0));
    assert_eq!(arrows(&f)[1].head, Head::TwoHeads);
}

#[test]
fn grid_then_label_statement() {
    let src = format!("{SQUARE}\nA \"\\mathbb{{Z}}\" color=blue\nD -> A dashed");
    let (f, d) = p(&src);
    no_errors(&d);
    assert_eq!(node(&f, "A").label, r"\mathbb{Z}");
    assert_eq!(node(&f, "A").color.as_deref(), Some("blue"));
    // the block is still written verbatim, with the label after it
    let out = format(&f);
    assert!(out.starts_with(SQUARE), "{out}");
    assert!(out.contains("A \"\\mathbb{Z}\" color=blue"));
    // a later `at` can't move a grid node
    let (_, d2) = p(&format!("{SQUARE}\nA at 5,5"));
    assert_eq!(warnings(&d2).len(), 1);
}

#[test]
fn grid_marks_without_nodes_warn() {
    let (f, d) = p("grid\nA --> B\n       |\n       v\n   C\nend");
    assert_eq!(warnings(&d).len(), 1);
    assert_eq!(arrows(&f).len(), 1);
}

#[test]
fn grid_comment_lines_inside() {
    let (f, d) = p("grid % the square\n  A --> B  % top\n  % nothing here\n  |     |\n  v     v\n  C --> D\nend");
    no_errors(&d);
    assert_eq!(arrows(&f).len(), 4);
}

#[test]
fn exact_sequence_has_two_zeros() {
    let (f, d) = p("grid\n0 --> K >--ι--> G --π->> Q --> 0\nend");
    no_errors(&d);
    assert_eq!(nodes(&f).len(), 5);
    assert_eq!(cell(&f, "0#2"), (4.0, 0.0));
    assert_eq!(node(&f, "0#2").label, "0");
    assert_eq!(edges(&f)[3], e("Q", "0#2", None));
}

#[test]
fn two_grids_share_a_node_by_name() {
    let (f, d) = p("grid\nA --> B\nend\ngrid\nB --> C\nend");
    assert_eq!(errors(&d).len(), 1);
    assert_eq!(edges(&f), [e("A", "B", None), e("B", "C", None)]);
}

// ---------------------------------------------------------------- errors and messy input

#[test]
fn error_recovery_keeps_the_rest() {
    let src = "A -> B\nfoo ->\n-> bar\nend\n\"x\" -> A\nC -> D\nblob\ncayley <a,b>";
    let (f, d) = p(src);
    let errs = errors(&d);
    assert_eq!(errs.len(), 6, "{errs:#?}");
    for (k, line) in [2, 3, 4, 5, 7, 8].iter().enumerate() {
        assert!(errs[k].message.starts_with(&format!("line {line}:")), "{}", errs[k].message);
        assert_eq!(errs[k].span.line, *line);
        assert!(errs[k].message.ends_with('.'));
    }
    assert_eq!(edges(&f), [e("A", "B", None), e("C", "D", None)]);
    // bad lines survive formatting as written
    let out = format(&f);
    assert!(out.contains("foo ->\n-> bar\nend\n\"x\" -> A"));
}

#[test]
fn messy_whitespace() {
    let (f, d) = p("   A    -f->     B   \n\n\n  ;  ;  C->D;;\n\t\tE");
    no_errors(&d);
    assert_eq!(arrows(&f).len(), 2);
    assert!(nodes(&f).iter().any(|n| n.name == "E"));
}

#[test]
fn garbage_never_panics() {
    let junk = [
        "\"", "\"\"\"", "{", "}", "{{{", "-> -> ->", "<", ">", "|", "||", "grid", "end", "grid\n|\nv", "grid\n\\\n/", "blob {",
        "blob G {a, {b}", "ngon", "ngon P x", "cayley", "cayley <", "cayley D4 <r", "text", "scale", "A at", "A at (", "A at (1,",
        "A -\"", "%", "\\", "A -> B bend=", "ngon P 4 axis", "ngon P 4 labels", "A \"x", "grid\n  A -- > \n end", "ŧ → ŋ",
        "A <-<- B", "A >->> B", "x^{ -> y", "A at 1,0,2", "blob G in", "ngon P 100000000000", "ngon P 4 axis 9.5 flip=x",
        "grid\nv\nA\n^", "grid\n\\pi\nA", "cayley Z2xZ <a>", "A =", "= B", "A ==>> B", "🙂 -> 🙃",
    ];
    for j in junk {
        let (f, _) = parse(j);
        // and formatting whatever came out doesn't panic either
        let _ = format(&f);
        p(j);
    }
}

#[test]
fn diagnostics_mention_line_numbers() {
    let (_, d) = p("A -> B\n\n\nngon P 2");
    let e = errors(&d);
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].span.line, 4);
    assert!(e[0].message.starts_with("line 4: "));
}

// (FIG-POLISH) a spaced product with no generators is one group, drawn with
// its standard generators, and written back as she spelled it
#[test]
fn cayley_spaced_products_without_generators() {
    for src in ["cayley Z2 x Z4", "cayley Z2 × Z4", "cayley ℤ2 × ℤ4", "cayley Z2 x Z4 legend"] {
        let (f, d) = p(src);
        no_errors(&d);
        let c = cayleys(&f)[0];
        assert_eq!(c.group, GroupSpec::Named("Z2xZ4".into()), "{src}");
        assert!(c.gens.is_empty(), "{src}");
        let g = ntex_figures::cayley::group(&c.group, &c.gens).unwrap();
        assert_eq!(g.order(), 8, "{src}");
        assert_eq!(g.gens.len(), 2, "{src}: the standard generators");
        assert!(format(&f).starts_with(src), "{src} → {}", format(&f));
    }
    let (f, _) = p("cayley Z2 x Z4 legend");
    assert!(cayleys(&f)[0].legend);
}
