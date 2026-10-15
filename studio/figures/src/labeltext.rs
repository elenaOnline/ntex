//! The text-mode editing surface for labels stored in TikZ's math mode.
//! Conversion is independent of snippet expansion (which belongs to ntex).

/// Show maths with delimiters (except pure digits), and text without its wrapper.
/// Call with `spelling::as_typed` / `spelling::raw` to retain her spelling.
pub fn field_from_label(label: &str) -> String {
    if label.is_empty() || pure_digits(label) {
        label.to_string()
    } else if let Some(text) = text_inside(label) {
        text.to_string()
    } else {
        format!("${label}$")
    }
}

fn pure_digits(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|c| c.is_ascii_digit())
}

fn text_inside(label: &str) -> Option<&str> {
    let inner = label.strip_prefix(r"\text{")?.strip_suffix('}')?;
    let mut depth = 0usize;
    let mut escaped = false;
    for c in inner.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' => escaped = true,
            '{' => depth += 1,
            '}' => depth = depth.checked_sub(1)?,
            _ => {}
        }
    }
    (depth == 0).then_some(inner)
}

/// Preserve Unicode spelling for the existing model edit/spelling pipeline.
pub fn label_from_field_spelling(text: &str) -> String {
    if text.is_empty() || pure_digits(text) {
        return text.to_string();
    }
    let mut dollars = Vec::new();
    let mut escaped = false;
    for (i, c) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
        } else if c == '$' {
            dollars.push(i);
        }
    }
    if dollars == [0, text.len() - 1] {
        text[1..text.len() - 1].to_string()
    } else {
        format!(r"\text{{{text}}}")
    }
}

/// Commit a field to the model's LaTeX representation.
pub fn label_from_field(text: &str) -> String {
    crate::unicode::to_latex(&label_from_field_spelling(text))
}

/// (figure-qol class C) How a stored label is edited: maths is the default
/// and shows without its `$…$`; only a `\text{…}` label is in text mode.
/// Returns (what the field holds, maths?). An empty label opens in maths.
/// Call with the label as she typed it (`spelling::as_typed`).
pub fn edit_form(label: &str) -> (String, bool) {
    if label.is_empty() || pure_digits(label) {
        (label.to_string(), true)
    } else if let Some(text) = text_inside(label) {
        (text.to_string(), false)
    } else {
        (label.to_string(), true)
    }
}

/// `text` between one outer pair of unescaped `$` (a pasted `$x$`), else None.
fn wrapped_in_dollars(text: &str) -> Option<&str> {
    let mut dollars = Vec::new();
    let mut escaped = false;
    for (i, c) in text.char_indices() {
        if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '$' {
            dollars.push(i);
        }
    }
    (text.len() >= 2 && dollars == [0, text.len() - 1]).then(|| &text[1..text.len() - 1])
}

/// The label for what the field holds in the given mode, keeping her
/// spelling (φ stays φ); the model's LaTeX is `label_from_edit`.
/// Maths: the text itself (a whole-field `$…$` loses its delimiters).
/// Text: `\text{…}`. Empty stays empty.
pub fn label_from_edit_spelling(text: &str, math: bool) -> String {
    if text.is_empty() {
        return String::new();
    }
    if math {
        wrapped_in_dollars(text).unwrap_or(text).to_string()
    } else {
        format!(r"\text{{{text}}}")
    }
}

/// Commit a field in the given mode to the model's LaTeX.
pub fn label_from_edit(text: &str, math: bool) -> String {
    crate::unicode::to_latex(&label_from_edit_spelling(text, math))
}

/// Free text items are already TeX text (TikZ does not add math delimiters
/// to them). Preserve their maths islands while converting Unicode once.
pub fn text_item_from_field(text: &str) -> String {
    let mut out = String::new();
    let mut chunk = String::new();
    let mut math = false;
    let mut escaped = false;
    for c in text.chars() {
        if c == '$' && !escaped {
            out.push_str(&if math {
                crate::unicode::to_latex(&chunk)
            } else {
                crate::unicode::to_latex_text(&chunk)
            });
            chunk.clear();
            out.push('$');
            math = !math;
        } else {
            chunk.push(c);
        }
        escaped = c == '\\' && !escaped;
    }
    out.push_str(&if math {
        crate::unicode::to_latex(&chunk)
    } else {
        crate::unicode::to_latex_text(&chunk)
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn text_by_default_and_explicit_math() {
        assert_eq!(label_from_field(""), "");
        assert_eq!(label_from_field("Fun fact"), r"\text{Fun fact}");
        assert_eq!(label_from_field(r"$\mathbb{Z}$"), r"\mathbb{Z}");
        assert_eq!(
            label_from_field(r"kernel of $\varphi$"),
            r"\text{kernel of $\varphi$}"
        );
        assert_eq!(label_from_field("$a$ $b$"), r"\text{$a$ $b$}");
        assert_eq!(label_from_field(r"\$5"), r"\text{\$5}");
    }
    #[test]
    fn round_trips() {
        for label in [
            "",
            "A",
            r"\mathbb{Z}",
            r"\frac{a}{b}",
            r"\text{Fun fact}",
            r"\text{kernel of $\varphi$}",
            r"\text{a {nested} word}",
            r"\text{\{escaped\}}",
        ] {
            assert_eq!(label_from_field(&field_from_label(label)), label);
        }
        assert_eq!(field_from_label(r"\text{a}+b"), r"$\text{a}+b$");
    }
    #[test]
    fn digit_math_labels_round_trip_without_delimiters() {
        for digits in ["0", "1", "12", "007", "1234567890"] {
            assert_eq!(field_from_label(digits), digits);
            assert_eq!(label_from_field(digits), digits);
            assert_eq!(label_from_field_spelling(digits), digits);
            assert_eq!(label_from_field(&field_from_label(digits)), digits);
            assert_eq!(field_from_label(&label_from_field(digits)), digits);
        }
        for label in ["x1", "-1", "1.5", "1 2", "١"] {
            assert_eq!(field_from_label(label), format!("${label}$"));
            assert_eq!(label_from_field(label), format!(r"\text{{{label}}}"));
        }
        assert_eq!(label_from_field("$1$"), "1");
    }

    #[test]
    fn maths_is_the_default_and_hides_its_delimiters() {
        assert_eq!(edit_form(""), (String::new(), true));
        assert_eq!(edit_form("A"), ("A".into(), true));
        assert_eq!(edit_form(r"\mathbb{Z}"), (r"\mathbb{Z}".into(), true));
        assert_eq!(edit_form("12"), ("12".into(), true));
        assert_eq!(edit_form(r"\text{Fun fact}"), ("Fun fact".into(), false));
        assert_eq!(edit_form(r"\text{a}+b"), (r"\text{a}+b".into(), true));
        assert_eq!(label_from_edit_spelling("f", true), "f");
        assert_eq!(label_from_edit_spelling("$f$", true), "f");
        assert_eq!(label_from_edit_spelling("Fun fact", false), r"\text{Fun fact}");
        assert_eq!(label_from_edit_spelling("", false), "");
        assert_eq!(label_from_edit(r"\ker φ", true), r"\ker \varphi");
        assert_eq!(label_from_edit("φ", true), r"\varphi");
        // the old whole-field forms agree with the new two-part ones
        for label in ["", "A", r"\mathbb{Z}", r"\frac{a}{b}", r"\text{Fun fact}", r"\text{kernel of $\varphi$}", "7"] {
            let (t, m) = edit_form(label);
            assert_eq!(label_from_edit(&t, m), label, "{label}");
            assert_eq!(label_from_edit(&t, m), label_from_field(&field_from_label(label)));
        }
    }

    #[test]
    fn free_text_items_keep_math_islands() {
        assert_eq!(text_item_from_field("Fun fact"), "Fun fact");
        assert_eq!(
            text_item_from_field("kernel of $φ$"),
            r"kernel of $\varphi$"
        );
        assert_eq!(text_item_from_field("$ℤ$"), r"$\mathbb{Z}$");
        assert_eq!(text_item_from_field("φ"), r"$\varphi$");
    }
    #[test]
    fn her_spelling() {
        assert_eq!(edit_form("φ"), ("φ".into(), true));
        assert_eq!(label_from_edit_spelling("φ", true), "φ");
        assert_eq!(label_from_edit("φ ℤ ∅ x₁", true), r"\varphi \mathbb{Z} \varnothing x_{1}");
        let fig = crate::parse::parse("A \"φ\"\n").0;
        let raw = crate::spelling::as_typed(&fig, &crate::model::ObjRef::Node("A".into())).unwrap();
        assert_eq!(field_from_label(&raw), "$φ$");
        assert_eq!(
            label_from_field("$φ ℤ ∅ x₁$"),
            r"\varphi \mathbb{Z} \varnothing x_{1}"
        );
        assert_eq!(label_from_field_spelling("$φ$"), "φ");
        assert_eq!(label_from_field("φ"), r"\text{\varphi}");
    }
}
