//! FIG-CORE-A: the formatter. Round trip on every spec example, fidelity
//! details, `item`, and a property test over generated sources.
mod core_a_common;
use core_a_common::*;

#[test]
fn every_code_block_in_the_spec_round_trips() {
    let spec = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/FIGURES.md")).unwrap();
    let shorthand = spec.split("## The shorthand").nth(1).unwrap().split("## Output").next().unwrap();
    let blocks: Vec<&str> = shorthand.split("```").skip(1).step_by(2).collect();
    assert!(blocks.len() >= 4);
    for b in &blocks {
        let (f, d) = p(b);
        assert!(errors(&d).is_empty(), "{b}\n{d:#?}");
        assert!(!f.items.is_empty());
    }
    // and every inline example in the shorthand section parses without errors
    let mut n = 0;
    for ex in shorthand.split('`').skip(1).step_by(2) {
        if ex.contains(" -> ") || ex.starts_with("text ") || ex.starts_with("A at") {
            let (_, d) = p(ex);
            assert!(errors(&d).is_empty(), "{ex}: {d:#?}");
            n += 1;
        }
    }
    assert!(n >= 3);
}

#[test]
fn canonical_source_is_a_fixed_point() {
    let src = "% header
spacing 2.8,2
grid
  A --f--> B
  |g       |h
  v        v
  C --k--> D
end
A \"\\mathbb{Z}\"
blob G {e, g, g^2, g^3}
blob K \"\\ker\\varphi\" {e, g^2} in G shaded
G -\\varphi-> H
g |-> h ; e |-> e' % elements
ngon P 5 labels a,b,c,d,e axes rotation
cayley D4 <r, s> legend
X -\"g \\circ f\"-> Y bend-left swap
text \"kernel $\\mapsto$ identity\" at (1,2)
";
    let (f, _) = p(src);
    assert_eq!(format(&f), src);
}

#[test]
fn implicit_nodes_stay_inline() {
    let (f, _) = p("A \"\\mathbb{Z}\" at 0,0 -> B");
    assert_eq!(format(&f), "A \"\\mathbb{Z}\" at 0,0 -> B\n");
}

#[test]
fn reversed_arrows_are_written_forwards() {
    let (f, _) = p("B <-f- A");
    assert_eq!(format(&f), "A -f-> B\n");
}

#[test]
fn semicolons_and_trailing_comments_keep_their_line() {
    let (f, _) = p("A -> B;C -> D   % two maps\nE");
    assert_eq!(format(&f), "A -> B ; C -> D % two maps\nE\n");
}

#[test]
fn globals_stay_where_she_put_them() {
    let (f, _) = p("% kernel\n% of phi\nA -> B\nscale 1.5\nsize large");
    assert_eq!(format(&f), "% kernel\n% of phi\nA -> B\nscale 1.5\nsize large\n");
    let (f, _) = p("scale 2\nA -> B\nspacing 3,2\nC -> D");
    assert_eq!(format(&f), "scale 2\nA -> B\nspacing 3,2\nC -> D\n");
    // a default value isn't written back; a global set twice keeps its last place
    let (f, _) = p("scale 1\nsize large\nA -> B\nsize small");
    assert_eq!(format(&f), "A -> B\nsize small\n");
}

#[test]
fn globals_with_no_place_go_after_the_header_comment() {
    let (mut f, _) = p("% kernel\nA -> B");
    f.globals.scale = 1.5;
    assert_eq!(format(&f), "% kernel\nscale 1.5\nA -> B\n");
}

#[test]
fn arrow_forms_are_canonical() {
    let cases = [
        ("A -f-> B", "A -f-> B"),
        ("A --f--> B", "A --f--> B"),
        ("A .f.> B", "A .f.> B"),
        ("A -f->> B", "A -f->> B"),
        ("A >-f-> B", "A >-f-> B"),
        ("A -f-hook-> B", "A -f-hook-> B"),
        ("A =f=> B", "A =f=> B"),
        ("A <-f-> B", "A <-f-> B"),
        ("A = B", "A = B"),
        ("a |-> b", "a |-> b"),
        ("A ↦ B", "A |-> B"),
        ("A -> B dashed", "A --> B"),
        ("A hook-> B dotted", "A hook-> B dotted"),
        ("A -\"g f\"-> B", "A -\"g f\"-> B"),
        ("A -\"x-1\"-> B", "A -\"x-1\"-> B"),
        ("A -> B bend", "A -> B bend-left"),
        ("A -> B bend=-12.5 near-start swap iso thick color=ochre", "A -> B bend=-12.5 swap near-start iso thick color=ochre"),
        ("A -> A loop", "A -> A"),
    ];
    for (src, want) in cases {
        let (f, _) = p(src);
        assert_eq!(format(&f), format!("{want}\n"), "{src}");
    }
}

#[test]
fn bad_lines_are_written_back_as_typed() {
    let src = "A -> B\nA -> ->\nC -> D ; ngon P 2 ; E\n";
    let (f, _) = p(src);
    assert_eq!(format(&f), src);
}

#[test]
fn item_gives_one_statement() {
    let (f, _) = p("grid\nA --> B\nend\nX \"x\" -> Y bend-left\nblob G {a, b} shaded\n% note");
    let texts: Vec<String> = (0..f.items.len()).map(|i| item(&f, i)).collect();
    assert_eq!(texts[0], "grid\nA --> B\nend");
    assert_eq!(texts[1], "A at 0,0"); // a grid node, as a statement
    assert_eq!(texts[3], "A -> B");
    assert_eq!(texts[4], "X \"x\""); // implicit node, standalone
    assert_eq!(texts[6], "X -> Y bend-left");
    assert_eq!(texts[7], "blob G {a, b} shaded");
    assert_eq!(texts[8], "% note");
    assert_eq!(item(&f, 99), "");
}

#[test]
fn a_grid_changed_outside_edit_is_rewritten_as_statements() {
    let (mut f, _) = p("% top\ngrid\nA --f--> B\n|       |\nv       v\nC -----> D\nend\nB -> A bend-left");
    // move D by hand (as a tidy pass might)
    for it in f.items.iter_mut() {
        if let Item::Node(n) = it {
            if n.name == "D" {
                n.pos = Some(Pos::Cell(2.0, 1.0));
            }
        }
    }
    let out = format(&f);
    assert!(out.contains("% grid (line 2) rewritten as statements"), "{out}");
    assert!(!out.contains("\nend"));
    let (g, d) = parse(&out);
    assert!(errors(&d).is_empty());
    assert_eq!(cell(&g, "D"), (2.0, 1.0));
    assert_eq!(cell(&g, "C"), (0.0, 1.0));
    assert_eq!(edges(&g), edges(&f));
}

#[test]
fn quotes_where_needed_in_lists() {
    let (f, _) = p("ngon P 3 labels \"a b\",c,d\nblob G {x \"\\{x\\}\"}");
    let out = format(&f);
    assert!(out.contains("labels \"a b\",c,d"), "{out}");
}

#[test]
fn without_spans_zeroes_every_span() {
    let (f, _) = p("A -> B\n% c\nblob G {x}\nngon P 3\ncayley Z3 <1>\ntext \"t\" at (0,0)\ngrid\nX\nend");
    let z = without_spans(&f);
    let json = serde_json::to_string(&z).unwrap();
    assert!(!json.contains("\"line\":1") && !json.contains("\"line\":2"));
}

// ---------------------------------------------------------------- property test

/// xorshift64*: deterministic, no dependencies.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545F4914F6CDD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn pick<'a>(&mut self, xs: &[&'a str]) -> &'a str {
        xs[self.below(xs.len())]
    }
    fn chance(&mut self, pct: usize) -> bool {
        self.below(100) < pct
    }
}

const NAMES: &[&str] = &["A", "B", "C", "D", "G", "H", "K", "G/K", "φ(G)", r"\ker\varphi", "ℤ", "Z_n", "H'", r"\mathbb{Z}", "x₁", "G^{ab}"];
const LABELS: &[&str] = &["f", "g", "π", r"\iota", "\"g \\circ f\"", "φ", "\"x-1\"", "h'", r"\pi_1", "≅"];
const OPTS: &[&str] = &["bend", "bend-left", "bend-right", "bend=40", "swap", "iso", "dashed", "dotted", "thick", "color=blue", "color=teal", "near-start", "near-end", "loop", "wobbly"];

fn arrow_tok(r: &mut Rng) -> String {
    let l = r.pick(LABELS);
    let lab = r.chance(50);
    match r.below(12) {
        0 => if lab { format!("-{l}->") } else { "->".into() },
        1 => if lab { format!("-{l}->>") } else { "->>".into() },
        2 => if lab { format!(">-{l}->") } else { ">->".into() },
        3 => if lab { format!("-{l}-hook->") } else { "hook->".into() },
        4 => if lab { format!("={l}=>") } else { "=>".into() },
        5 => if lab { format!("--{l}-->") } else { "-->".into() },
        6 => if lab { format!(".{l}.>") } else { "..>".into() },
        7 => if lab { format!("<-{l}-") } else { "<-".into() },
        8 => if lab { format!("<-{l}->") } else { "<->".into() },
        9 => "=".into(),
        10 => "|->".into(),
        _ => r.pick(&["→", "↦", "↠", "↪"]).into(),
    }
}

fn pos(r: &mut Rng) -> String {
    match r.below(4) {
        0 => format!("at {},{}", r.below(4), r.below(3)),
        1 => format!("at {}, {}", r.below(4), r.below(3)),
        2 => format!("at ({}.5,-{})", r.below(5), r.below(3)),
        _ => format!("at 0.5,{}", r.below(3)),
    }
}

fn term(r: &mut Rng) -> String {
    let mut s = r.pick(NAMES).to_string();
    if r.chance(15) {
        s.push_str(&format!(" \"{}\"", r.pick(&[r"\mathbb{Z}", "X", "", "G_1"])));
    }
    if r.chance(10) {
        s.push(' ');
        s.push_str(&pos(r));
    }
    s
}

fn statement(r: &mut Rng, blobs: &mut Vec<String>) -> String {
    match r.below(20) {
        0..=7 => {
            let mut s = term(r);
            for _ in 0..1 + r.below(2) {
                s.push_str(&format!(" {} {}", arrow_tok(r), term(r)));
            }
            for _ in 0..r.below(3) {
                s.push(' ');
                s.push_str(r.pick(OPTS));
            }
            s
        }
        8..=9 => {
            let mut s = term(r);
            if r.chance(40) {
                s.push_str(" color=maroon");
            }
            s
        }
        10..=11 => {
            let name = format!("B{}", blobs.len());
            let k = blobs.len();
            let mut s = format!("blob {name}");
            if r.chance(50) {
                s.push_str(" \"\\ker\\varphi\"");
            }
            let els: Vec<String> = (0..r.below(4)).map(|j| format!("{}{}", r.pick(&["e", "g", "x", "y"]), j + 10 * k)).collect();
            if !els.is_empty() || r.chance(30) {
                s.push_str(&format!(" {{{}}}", els.join(", ")));
            }
            if !blobs.is_empty() && r.chance(50) {
                s.push_str(&format!(" in {}", blobs[r.below(blobs.len())]));
            }
            for _ in 0..r.below(3) {
                s.push(' ');
                s.push_str(r.pick(&["shaded", "color=green", "shape=tall", "size=2,1.5", "label=below", "at (1,1)", "bogus"]));
            }
            blobs.push(name);
            s
        }
        12 => {
            let n = 3 + r.below(5);
            let mut s = format!("ngon {} {}", r.pick(&["P", "Q", "R"]), n);
            for _ in 0..r.below(4) {
                s.push(' ');
                s.push_str(&match r.below(7) {
                    0 => "axes".to_string(),
                    1 => format!("axis {}", 1 + r.below(n)),
                    2 => format!("axis {}.5", 1 + r.below(n)),
                    3 => "rotation".into(),
                    4 => format!("rotation={}", 1 + r.below(3)),
                    5 => format!("turn={} flip={}", r.below(3), 1 + r.below(n)),
                    _ => "radius=1.2".into(),
                });
            }
            s
        }
        13 => r.pick(&["cayley D4 <r, s>", "cayley <a, b | a^4, b^2, (ab)^2> legend", "cayley Z6 <1> layout=circle", "cayley Z2 x Z3 <a, b> labels=none", "cayley Q8 <i, j>"]).into(),
        14 => format!("text \"{}\" {}", r.pick(&["kernel ↦ identity", "a b", "x"]), pos(r)),
        15 => r.pick(&["scale 1.2", "spacing 2.8,2", "size small", "scale 0"]).into(),
        16 => r.pick(&["% a comment", "", "%"]).into(),
        17 => r.pick(&["-> X", "A ->", "\"q\" -> B", "ngon P 1", "end", "blob {x}", "A at nowhere"]).into(),
        18 => format!("{} -> {}", r.pick(&["P.1", "D4.r", "e0", "g0", "B0"]), r.pick(&["P.2", "Q.1", "x10", "B1"])),
        _ => format!("{} {}", term(r), r.pick(&["% trailing", "; X -> Y"])),
    }
}

fn grid(r: &mut Rng) -> String {
    let rows = 1 + r.below(3);
    let cols = 1 + r.below(3);
    let names = ["P", "Q", "R", "S", "T", "U", "V", "W", "Y"];
    let w = 9;
    let mut lines = vec!["grid".to_string()];
    for row in 0..rows {
        let mut line = String::new();
        for c in 0..cols {
            let nm = names[row * 3 + c];
            line.push_str(nm);
            if c + 1 < cols {
                let a = r.pick(&["--f-->", "------>", "<--g--", "-->>  ", ">---> ", "- - > ", "..>   ", "  =   "]);
                line.push(' ');
                line.push_str(a);
                while line.chars().count() < (c + 1) * w {
                    line.push(' ');
                }
            }
        }
        lines.push(line);
        if row + 1 < rows {
            let mut a = String::new();
            let mut b = String::new();
            for c in 0..cols {
                let (x, y) = match r.below(5) {
                    0 => ("|g", "v "),
                    1 => ("^ ", "|h"),
                    2 => (": ", "v "),
                    3 => ("‖ ", "‖ "),
                    _ => ("  ", "  "),
                };
                while a.chars().count() < c * w {
                    a.push(' ');
                    b.push(' ');
                }
                a.push_str(x);
                b.push_str(y);
            }
            lines.push(a.trim_end().to_string());
            lines.push(b.trim_end().to_string());
        }
    }
    lines.push("end".into());
    lines.join("\n")
}

fn source(r: &mut Rng) -> String {
    let mut blobs = Vec::new();
    let mut lines = Vec::new();
    for _ in 0..1 + r.below(12) {
        if r.chance(10) {
            lines.push(grid(r));
        } else if r.chance(10) {
            let a = statement(r, &mut blobs);
            let b = statement(r, &mut blobs);
            lines.push(format!("{a} ; {b}"));
        } else {
            lines.push(statement(r, &mut blobs));
        }
    }
    lines.join("\n")
}

#[test]
fn property_round_trip_over_generated_sources() {
    let mut r = Rng(0x9E3779B97F4A7C15);
    let mut grids = 0;
    for case in 0..1500 {
        let src = source(&mut r);
        if src.contains("grid") {
            grids += 1;
        }
        // p() asserts the round trip and idempotence
        let result = std::panic::catch_unwind(|| p(&src));
        assert!(result.is_ok(), "case {case} failed:\n{src}");
    }
    assert!(grids > 100);
}

#[test]
#[ignore]
fn show_generated_samples() {
    let mut r = Rng(0x9E3779B97F4A7C15);
    let (mut items, mut errs, mut arrows_n, mut grid_arrows) = (0, 0, 0, 0);
    for k in 0..1500 {
        let src = source(&mut r);
        let (f, d) = parse(&src);
        items += f.items.len();
        errs += errors(&d).len();
        arrows_n += arrows(&f).len();
        grid_arrows += f.grid_items.len();
        if k < 3 {
            println!("=== source\n{src}\n=== formatted\n{}", format(&f));
        }
    }
    println!("items {items} errors {errs} arrows {arrows_n} grid-derived {grid_arrows}");
}

// ---------------------------------------------------------------- her spelling (FIG-POLISH)

#[test]
fn her_spelling_survives_every_rewrite() {
    let src = "% spelling\n\
A \"φ(G)\" -π-> B \"G/K\"\n\
C \"\\varphi(G)\"\n\
A -\"g ∘ f\"-> C\n\
blob G \"ℤ\" {x \"x₁\", y}\n\
ngon P 3 labels α,β,\\gamma\n\
cayley <ρ, σ | ρ³, σ², (ρσ)²>\n\
text \"kernel ↦ e\" at (1,2)\n\
grid\n  X --ψ--> Y\nend\n\
size large\n";
    let (f, _) = p(src);
    assert_eq!(node(&f, "A").label, "\\varphi(G)");
    let out = format(&f);
    for s in ["A \"φ(G)\" -π-> B", "C \"\\varphi(G)\"", "-\"g ∘ f\"->", "blob G \"ℤ\" {x \"x₁\", y}", "labels α,β,\\gamma", "<ρ, σ | ρ³, σ², (ρσ)²>", "text \"kernel ↦ e\"", "X --ψ--> Y"] {
        assert!(out.contains(s), "lost `{s}`:\n{out}");
    }
    assert!(out.ends_with("end\nsize large\n"), "the global moved:\n{out}");
    // an edit elsewhere keeps every spelling; a moved node too
    let mut g = f.clone();
    apply(&mut g, &Edit::MoveNode { name: "A".into(), pos: Pos::Cell(2.0, 2.0) }).unwrap();
    apply(&mut g, &Edit::AddNode { label: "ψ ∘ φ".into(), pos: Pos::Cell(3.0, 0.0) }).unwrap();
    let out2 = format(&g);
    assert!(out2.contains("A \"φ(G)\" at 2,2 -π-> B"), "{out2}");
    assert!(out2.contains("\"ψ ∘ φ\" at 3,0"), "{out2}");
    let (h, _) = parse(&out2);
    assert_eq!(without_spans(&h), without_spans(&g));
    // converting the grid writes its labels as she typed them
    apply(&mut g, &Edit::MoveNode { name: "X".into(), pos: Pos::Cell(0.0, 3.0) }).unwrap();
    let out3 = format(&g);
    assert!(out3.contains("X -ψ-> Y"), "{out3}");
    // a changed label is written as the new text; an unchanged one keeps hers
    apply(&mut g, &Edit::SetNodeLabel { name: "C".into(), label: "ε".into() }).unwrap();
    let out4 = format(&g);
    assert!(out4.contains("C \"ε\""), "{out4}");
    let (h, _) = parse(&out4);
    assert_eq!(without_spans(&h), without_spans(&g));
}

#[test]
fn spelling_hints_name_their_sites() {
    let (f, _) = p("A -φ-> B\nA -\\varphi-> C");
    let keys: Vec<(&str, &str)> = f.hints.spelling.iter().map(|s| (s.key.as_str(), s.raw.as_str())).collect();
    assert_eq!(keys, [("arrow:A>B", "φ")]);
    assert_eq!(format(&f), "A -φ-> B\nA -\\varphi-> C\n");
}
