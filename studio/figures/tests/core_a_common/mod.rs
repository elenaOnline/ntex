//! Shared helpers for FIG-CORE-A's tests.
#![allow(dead_code)]
pub use ntex_figures::edit::{apply, fresh_name};
pub use ntex_figures::format::{format, item, without_spans};
pub use ntex_figures::model::*;
pub use ntex_figures::parse::parse;

/// Parse, check the round trip `parse(format(parse(s))) == parse(s)` (spans
/// aside) and that formatting is idempotent; return the figure + diagnostics.
pub fn p(src: &str) -> (Figure, Vec<Diagnostic>) {
    let (f, d) = parse(src);
    let out = format(&f);
    let (g, _) = parse(&out);
    assert_eq!(without_spans(&f), without_spans(&g), "round trip failed\n--- source:\n{src}\n--- formatted:\n{out}");
    assert_eq!(format(&g), out, "format isn't idempotent\n--- source:\n{src}");
    (f, d)
}

pub fn nodes(f: &Figure) -> Vec<&Node> {
    f.items.iter().filter_map(|i| if let Item::Node(n) = i { Some(n) } else { None }).collect()
}
pub fn arrows(f: &Figure) -> Vec<&Arrow> {
    f.items.iter().filter_map(|i| if let Item::Arrow(a) = i { Some(a) } else { None }).collect()
}
pub fn blobs(f: &Figure) -> Vec<&Blob> {
    f.items.iter().filter_map(|i| if let Item::Blob(b) = i { Some(b) } else { None }).collect()
}
pub fn ngons(f: &Figure) -> Vec<&NGon> {
    f.items.iter().filter_map(|i| if let Item::NGon(g) = i { Some(g) } else { None }).collect()
}
pub fn cayleys(f: &Figure) -> Vec<&Cayley> {
    f.items.iter().filter_map(|i| if let Item::Cayley(c) = i { Some(c) } else { None }).collect()
}
pub fn node<'a>(f: &'a Figure, name: &str) -> &'a Node {
    nodes(f).into_iter().find(|n| n.name == name).unwrap_or_else(|| panic!("no node {name} in {:#?}", f.items))
}
pub fn cell(f: &Figure, name: &str) -> (f32, f32) {
    match node(f, name).pos {
        Some(Pos::Cell(x, y)) => (x, y),
        other => panic!("{name} has no cell: {other:?}"),
    }
}
/// (from, to, label) of every arrow
pub fn edges(f: &Figure) -> Vec<(String, String, Option<String>)> {
    arrows(f).iter().map(|a| (a.from.clone(), a.to.clone(), a.label.clone())).collect()
}
pub fn e(from: &str, to: &str, l: Option<&str>) -> (String, String, Option<String>) {
    (from.into(), to.into(), l.map(|s| s.into()))
}
pub fn errors(d: &[Diagnostic]) -> Vec<&Diagnostic> {
    d.iter().filter(|x| x.severity == Severity::Error).collect()
}
pub fn warnings(d: &[Diagnostic]) -> Vec<&Diagnostic> {
    d.iter().filter(|x| x.severity == Severity::Warning).collect()
}
pub fn no_errors(d: &[Diagnostic]) {
    let e: Vec<_> = d.iter().filter(|x| x.severity != Severity::Note).collect();
    assert!(e.is_empty(), "unexpected diagnostics: {e:#?}");
}
