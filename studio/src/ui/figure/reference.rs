//! ⌘/ — the Reference panel (FIGURES.md "Help instead of a cheat sheet in
//! the file"): the shorthand's cheat sheet as a Remark box over the
//! shorthand box, instead of 50 lines of comments in every new file.
//!
//! The text is `figures/reference.md` (FIG-MODEL's, `build::REFERENCE`); if
//! it is ever empty, the window's own copy below.
use crate::ui::widgets::boxes::{BoxKind, Rich, TBox};
use crate::ui::widgets::theme::Theme;
use gpui::{AnyElement, IntoElement, ParentElement as _, Styled as _, div, px};

/// FIG-MODEL's `figures/reference.md` (ntex_figures::build::REFERENCE).
const EMBEDDED: &str = ntex_figures::build::REFERENCE;

/// Until figures/reference.md exists: the cheat sheet the template carried.
const FALLBACK: &str = r#"# The shorthand

## Arrows: `FROM <arrow> TO [options]`
```
A -> B          A -f-> B          A -"g \circ f"-> B
A ->> B  onto    A >-> B  into     A hook-> B  inclusion
A => B   double  A --> B  dashed   A ..> B     dotted
B <-f- A         A <-> B           A = B       g |-> h   (or ↦)
```
Options: `bend` `bend-left` `bend-right` `bend=40` `loop` `swap` `iso` `dashed` `dotted` `thick` `color=blue|violet|teal|maroon|green|ochre|grey|ink` `near-start` `near-end`

## Placing
`A at 1,0` (column, row; rows go down) · `A at (3.2,-1)` (cm, y up) · no position: placed for you · `spacing 2.8,2` · `scale 1.2` · `size small|normal|large`

## Labels
`A "\mathbb{Z}/n\mathbb{Z}"` · Unicode is fine: φ ℤ ∘ ≅ x₁ g²

## A square, as ASCII art
```
grid
  A --f--> B
  |g       |h
  v        v
  C --k--> D
end
```

## Blobs (sets and functions)
```
blob G "G" {e, g, g^2, g^3}
blob K "\ker\varphi" in G shaded {e, g^2}
G -φ-> H ; g |-> h
```
Options: `shaded` `color=…` `shape=round|tall|wide|bean` `size=w,h` `angle=θ` `at …` `label=above|below|left|right|inside`

## N-gons (dihedral pictures)
`ngon P 4` · `labels a,b,c,d` (`labels=none` hides them, `labels 1,,3,4` one) · `axes` · `axis k` / `axis k.5` · `rotation[=k]` (`rotation cw`, `rotation=-k`, `rotation edge=3`) · `turn=k` (`turn=-1`) · `flip=k` · `radius=r` · `angle=θ` · `plain` · `color=…` · `at …`

## Cayley graphs
`cayley D4 <r, s> legend` · `cayley <a, b | a^3, b^2, abab>` · `layout=circle|prism|grid` · `labels=none` · `gens r^-1:maroon,s:hidden` · `radius=r` · `angle=θ`

## Text
`text "kernel ↦ identity" at (0,-2)`
"#;

/// The reference text (markdown).
pub fn text() -> &'static str {
    if EMBEDDED.trim().is_empty() { FALLBACK } else { EMBEDDED }
}

/// Is the reference FIG-MODEL's file (not the window's own copy)?
pub fn from_file() -> bool {
    !EMBEDDED.trim().is_empty()
}

/// A line of prose with `code` spans in mono.
fn inline(t: &Theme, line: &str) -> AnyElement {
    let mut r = Rich::new();
    for (i, part) in line.split('`').enumerate() {
        if part.is_empty() {
            continue;
        }
        if i % 2 == 1 {
            r = r.span(part, t.mono(), t.ink);
            continue;
        }
        // **bold** and *italic* outside code
        for (j, b) in part.split("**").enumerate() {
            if b.is_empty() {
                continue;
            }
            if j % 2 == 1 {
                r = r.span(b, t.serif_bold(), t.ink);
            } else {
                for (k, it) in b.split('*').enumerate() {
                    if !it.is_empty() {
                        r = r.span(it, if k % 2 == 1 { t.serif_italic() } else { t.serif() }, t.ink);
                    }
                }
            }
        }
    }
    div().child(r.build()).into_any_element()
}

/// The Remark box: headings bold, code blocks in mono, `code` spans in mono.
pub fn render(t: &Theme) -> TBox {
    let mut body: Vec<AnyElement> = Vec::new();
    let mut code: Option<Vec<String>> = None;
    for line in text().lines() {
        if line.trim_start().starts_with("```") {
            match code.take() {
                Some(lines) => body.push(
                    div()
                        .flex()
                        .flex_col()
                        .pl(px(14.))
                        .font(t.mono())
                        .text_size(t.mono_size)
                        .line_height(t.line(t.mono_size))
                        .children(lines.into_iter().map(|l| div().child(if l.is_empty() { " ".to_string() } else { l })))
                        .into_any_element(),
                ),
                None => code = Some(Vec::new()),
            }
            continue;
        }
        if let Some(c) = code.as_mut() {
            c.push(line.to_string());
            continue;
        }
        let l = line.trim();
        if l.is_empty() {
            continue;
        }
        if let Some(h) = l.strip_prefix("# ") {
            // the box head already says "Reference"; a top heading is its subtitle
            body.push(div().font(t.serif_italic()).text_color(t.slate).child(h.to_string()).into_any_element());
        } else if let Some(h) = l.strip_prefix("## ").or_else(|| l.strip_prefix("### ")) {
            body.push(div().pt(px(4.)).font(t.serif_bold()).child(h.replace('`', "")).into_any_element());
        } else if let Some(b) = l.strip_prefix("- ") {
            body.push(div().flex().flex_row().gap(px(6.)).child("•").child(inline(t, b)).into_any_element());
        } else {
            body.push(inline(t, l));
        }
    }
    if let Some(lines) = code {
        body.push(div().font(t.mono()).children(lines.into_iter().map(|l| div().child(l))).into_any_element());
    }
    TBox::new(BoxKind::Remark, "Remark").name("Reference").own_line().children(body)
}

/// The first line of the source that isn't a comment or blank (0-based),
/// where the shorthand box scrolls on opening.
pub fn first_code_line(src: &str) -> usize {
    src.lines().position(|l| {
        let l = l.trim();
        !l.is_empty() && !l.starts_with('%')
    })
    // all comments: the end, where her first line goes
    .unwrap_or_else(|| src.lines().count().saturating_sub(1))
}
