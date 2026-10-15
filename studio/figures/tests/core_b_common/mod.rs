//! FIG-CORE-B test helpers: the spec examples built by hand (until/unless
//! `parse` is available), and headless rasterising.
#![allow(dead_code)]
use ntex_figures::model::*;
use std::path::{Path, PathBuf};

pub fn sp() -> Span {
    Span { line: 1, col: 1 }
}

pub fn node(name: &str, label: &str, pos: Option<Pos>) -> Item {
    Item::Node(Node { name: name.into(), label: label.into(), pos, color: None, span: sp() })
}

pub fn arr(from: &str, to: &str, label: Option<&str>) -> Arrow {
    Arrow {
        from: from.into(),
        to: to.into(),
        label: label.map(|s| s.to_string()),
        head: Head::To,
        line: Line::Solid,
        bend: 0.0,
        is_loop: false,
        side: LabelSide::Auto,
        label_pos: LabelPos::Middle,
        iso: false,
        thick: false,
        color: None,
        span: sp(),
    }
}

pub fn a(from: &str, to: &str, label: Option<&str>) -> Item {
    Item::Arrow(arr(from, to, label))
}

pub fn with(mut x: Arrow, f: impl FnOnce(&mut Arrow)) -> Item {
    f(&mut x);
    Item::Arrow(x)
}

pub fn el(name: &str) -> Element {
    Element { name: name.into(), label: name.into(), pos: None }
}

pub fn blob(name: &str, label: &str, elems: &[&str], parent: Option<&str>, shaded: bool) -> Blob {
    Blob {
        name: name.into(),
        label: label.into(),
        elements: elems.iter().map(|e| el(e)).collect(),
        parent: parent.map(|s| s.into()),
        shaded,
        color: None,
        shape: BlobShape::Round,
        size: None,
        pos: None,
        label_place: LabelPlace::Above,
        span: sp(),
        angle: 0.0,
    }
}

pub fn ngon(name: &str, n: u32) -> NGon {
    NGon { name: name.into(), n, labels: (1..=n).map(|i| i.to_string()).collect(), all_axes: false, axes: vec![], rotation: 0, turn: 0, flip: None, radius: None, pos: None, span: sp(), angle: 0.0, show_labels: true, shaded: true, color: None, rotation_cw: false, rotation_edge: None, turn_neg: false, rotation_label: None, axis_labels: vec![] }
}

pub fn cayley(name: &str, group: GroupSpec, gens: &[&str], legend: bool) -> Item {
    Item::Cayley(Cayley { name: name.into(), group, gens: gens.iter().map(|s| s.to_string()).collect(), layout: CayleyLayout::Auto, labels: true, legend, radius: None, pos: None, span: sp(), angle: 0.0, gen_styles: vec![], vertex_names: vec![] })
}

pub fn fig(items: Vec<Item>) -> Figure {
    Figure { items, ..Default::default() }
}

pub fn maps(x: &str, y: &str) -> Item {
    with(arr(x, y, None), |a| a.head = Head::MapsTo)
}

/// The spec examples, by hand, keyed by their figures/examples/*.fig stem.
pub fn examples() -> Vec<(&'static str, Figure)> {
    let mut v = vec![];
    v.push(("01-square", fig(vec![a("A", "B", Some("f")), a("A", "C", Some("g")), a("B", "D", Some("h")), a("C", "D", Some("k"))])));
    v.push((
        "02-square-grid",
        fig(vec![
            node("A", "A", Some(Pos::Cell(0.0, 0.0))),
            node("B", "B", Some(Pos::Cell(1.0, 0.0))),
            node("C", "C", Some(Pos::Cell(0.0, 1.0))),
            node("D", "D", Some(Pos::Cell(1.0, 1.0))),
            a("A", "B", Some("f")),
            a("A", "C", Some("g")),
            a("B", "D", Some("h")),
            a("C", "D", Some("k")),
        ]),
    ));
    v.push((
        "03-pullback",
        fig(vec![
            node("W", "W", Some(Pos::Cell(0.0, 0.0))),
            node("P", "X \\times_Z Y", Some(Pos::Cell(1.0, 1.0))),
            node("X", "X", Some(Pos::Cell(2.0, 1.0))),
            node("Y", "Y", Some(Pos::Cell(1.0, 2.0))),
            node("Z", "Z", Some(Pos::Cell(2.0, 2.0))),
            a("P", "X", Some("p_1")),
            with(arr("P", "Y", Some("p_2")), |a| a.side = LabelSide::Swap),
            a("X", "Z", Some("f")),
            with(arr("Y", "Z", Some("g")), |a| a.side = LabelSide::Swap),
            with(arr("W", "P", Some("\\exists!")), |a| a.line = Line::Dashed),
            with(arr("W", "X", Some("a")), |a| a.bend = 30.0),
            with(arr("W", "Y", Some("b")), |a| {
                a.bend = -30.0;
                a.side = LabelSide::Swap
            }),
        ]),
    ));
    v.push((
        "04-first-iso",
        fig(vec![
            with(arr("G", "G/K", Some("\\pi")), |a| a.head = Head::TwoHeads),
            with(arr("G/K", "\\varphi(G)", Some("\\bar\\varphi")), |a| a.iso = true),
            with(arr("G", "\\varphi(G)", Some("\\varphi")), |a| a.side = LabelSide::Swap),
        ]),
    ));
    v.push((
        "05-function-blobs",
        fig(vec![
            Item::Blob(blob("G", "G", &["e", "g", "g^2", "g^3"], None, false)),
            Item::Blob(Blob { elements: vec![el("e'"), el("h")], ..blob("H", "H", &[], None, false) }),
            a("G", "H", Some("\\varphi")),
            maps("e", "e'"),
            maps("g", "h"),
            maps("g^2", "e'"),
            maps("g^3", "h"),
        ]),
    ));
    v.push((
        "06-kernel",
        fig(vec![
            Item::Blob(blob("G", "G", &["g", "gk"], None, false)),
            Item::Blob(blob("K", "\\ker\\varphi", &["e", "k"], Some("G"), true)),
            Item::Blob(blob("H", "H", &["e'", "h"], None, false)),
            a("G", "H", Some("\\varphi")),
            maps("e", "e'"),
            maps("k", "e'"),
            maps("g", "h"),
            maps("gk", "h"),
        ]),
    ));
    v.push(("07-d4-square", fig(vec![Item::NGon(NGon { all_axes: true, rotation: 1, ..ngon("P", 4) })])));
    v.push((
        "08-d5-pentagon",
        fig(vec![Item::NGon(NGon { all_axes: true, rotation: 2, labels: ["a", "b", "c", "d", "e"].iter().map(|s| s.to_string()).collect(), ..ngon("P", 5) })]),
    ));
    v.push((
        "09-d4-sequence",
        fig(vec![Item::NGon(NGon { axes: vec![4], ..ngon("P", 4) }), Item::NGon(NGon { turn: 1, ..ngon("Q", 4) }), Item::NGon(NGon { flip: Some(4), axes: vec![4], ..ngon("R", 4) })]),
    ));
    v.push(("10-cayley-d4", fig(vec![cayley("C", GroupSpec::Named("D4".into()), &["r", "s"], true)])));
    v.push(("11-cayley-q8", fig(vec![cayley("C", GroupSpec::Named("Q8".into()), &["i", "j"], true)])));
    v.push((
        "12-cayley-presentation",
        fig(vec![cayley("C", GroupSpec::Presentation(vec!["a".into(), "b".into()], vec!["a^3".into(), "b^2".into(), "abab".into()]), &["a", "b"], true)]),
    ));
    v.push(("13-cayley-z6", fig(vec![cayley("C", GroupSpec::Named("Z6".into()), &["1"], false)])));
    v.push((
        "14-quick",
        fig(vec![with(arr("N", "G", None), |a| a.head = Head::Hook), with(arr("G", "G/N", None), |a| a.head = Head::TwoHeads), a("G", "H", Some("\\varphi"))]),
    ));
    v.push((
        "15-exact",
        fig(vec![
            node("0'", "0", None),
            a("0", "A", None),
            with(arr("A", "B", Some("\\iota")), |a| a.head = Head::Tail),
            with(arr("B", "C", Some("\\pi")), |a| a.head = Head::TwoHeads),
            a("C", "0'", None),
        ]),
    ));
    v.push(("17-cayley-a4", fig(vec![cayley("C", GroupSpec::Named("A4".into()), &["a", "b"], true)])));
    v.push(("16-cayley-z2xz3", fig(vec![cayley("C", GroupSpec::Named("Z2xZ3".into()), &["a", "b"], true)])));
    // (FIG-POLISH) arrows both ways side by side, and tikz-cd's `equal`
    v.push((
        "18-both-ways-equal",
        fig(vec![
            node("A", "A", Some(Pos::Cell(0.0, 0.0))),
            node("B", "B", Some(Pos::Cell(1.0, 0.0))),
            node("C", "C", Some(Pos::Cell(1.0, 1.0))),
            a("A", "B", Some("f")),
            with(arr("B", "A", Some("g")), |a| a.head = Head::TwoHeads),
            with(arr("B", "C", None), |a| a.head = Head::Equal),
        ]),
    ));
    // (FIG-MODEL, interaction pass) angle, hidden labels, plain, colour
    v.push(("19-ngon-turned-no-labels", fig(vec![Item::NGon(NGon { angle: 45.0, show_labels: false, ..ngon("P", 4) }), Item::NGon(NGon { angle: 45.0, all_axes: true, rotation: 1, ..ngon("Q", 4) })])));
    v.push(("20-blob-turned", fig(vec![Item::Blob(Blob { angle: 35.0, ..blob("G", "G", &["e", "g", "g^2", "g^3"], None, false) }), Item::Blob(Blob { angle: -20.0, ..blob("H", "H", &["e'", "h"], None, false) })])));
    v.push((
        "21-hexagon-plain-colour",
        fig(vec![Item::NGon(NGon { shaded: false, color: Some("maroon".into()), labels: ["1", "2", "", "4", "5", "6"].iter().map(|s| s.to_string()).collect(), ..ngon("P", 6) })]),
    ));
    v.push(("22-cayley-d4-turned", fig(vec![{
        let mut c = cayley("C", GroupSpec::Named("D4".into()), &["r", "s"], true);
        if let Item::Cayley(x) = &mut c {
            x.angle = 22.5;
        }
        c
    }])));
    v
}

pub fn studio_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}
pub fn figures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
pub fn templates_tex() -> PathBuf {
    // the real theme, read-only (tests never write it)
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap();
    home.join(".config/texnvim/templates/tex")
}

pub fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ntex-fig-coreb-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The headless CoreGraphics rasteriser (figures/evidence/pdf2png.swift),
/// compiled once into target/. None if swiftc isn't there.
pub fn pdf2png() -> Option<PathBuf> {
    let src = figures_dir().join("evidence/pdf2png.swift");
    let out = studio_dir().join("target/ntex-fig-pdf2png");
    let fresh = out.exists() && std::fs::metadata(&out).and_then(|m| m.modified()).ok() >= std::fs::metadata(&src).and_then(|m| m.modified()).ok();
    if !fresh {
        let st = std::process::Command::new("swiftc").arg("-O").arg("-o").arg(&out).arg(&src).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().ok()?;
        if !st.success() {
            return None;
        }
    }
    Some(out)
}

pub fn rasterise(pdf: &Path, png: &Path, scale: f32) -> Result<(), String> {
    let tool = pdf2png().ok_or("no swiftc")?;
    let st = std::process::Command::new(tool).arg(pdf).arg(png).arg(format!("{scale}")).stdout(std::process::Stdio::null()).status().map_err(|e| e.to_string())?;
    if st.success() { Ok(()) } else { Err(format!("pdf2png failed on {}", pdf.display())) }
}

/// Rasterise and return the PNG's pixel size (the tool prints `WxH`).
pub fn rasterise_size(pdf: &Path, png: &Path, scale: f32) -> Result<(u32, u32), String> {
    let tool = pdf2png().ok_or("no swiftc")?;
    let o = std::process::Command::new(tool).arg(pdf).arg(png).arg(format!("{scale}")).output().map_err(|e| e.to_string())?;
    let s = String::from_utf8_lossy(&o.stdout);
    let (w, h) = s.trim().split_once('x').ok_or("no size")?;
    Ok((w.parse().map_err(|_| "bad w")?, h.parse().map_err(|_| "bad h")?))
}
