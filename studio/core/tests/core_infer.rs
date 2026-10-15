//! infer (guess, check, try_plan, the Lua pattern matcher) and words, against
//! a known list shaped like her real snippets (what the bridge's `list` gives).
use ntex_studio_core::codegen::parse_body;
use ntex_studio_core::infer::{self, check, guess, lua_find, lua_pattern_check, pattern_example, try_plan};
use ntex_studio_core::model::*;
use ntex_studio_core::words;

fn t(s: &str) -> Piece {
    Piece::Text(s.into())
}
fn tab(n: u32) -> Piece {
    Piece::Tab { n, default: String::new() }
}
fn tabd(n: u32, d: &str) -> Piece {
    Piece::Tab { n, default: d.into() }
}
fn draft(trigger: &str, output: Vec<Piece>) -> Draft {
    Draft { trigger: trigger.into(), output, ..Draft::default() }
}
fn with(trigger: &str, output: Vec<Piece>, f: impl FnOnce(&mut Draft)) -> Draft {
    let mut d = draft(trigger, output);
    f(&mut d);
    d
}

/// A KnownSnippet like the bridge reports: (trigger, kind, where, how, body, file).
fn k(trigger: &str, kind: &str, where_: &str, how: &str, body: &str, file: &str) -> KnownSnippet {
    KnownSnippet {
        trigger: trigger.into(),
        pattern: None,
        kind: kind.into(),
        where_: where_.into(),
        how: how.into(),
        body: Some(body.into()),
        file: file.into(),
        line: None,
        origin: "yours".into(),
        group: None,
        line_rule: None,
        ft: None,
        priority: 0,
        desc: None,
    }
}

/// A few of her real snippets (sets, operations, symbols, greek, calculus,
/// text, accents, subscripts, visual), as `require("snips").list()` has them.
fn known() -> Vec<KnownSnippet> {
    let mut v = vec![
        k("uu", "math", "math", "auto", r"\cup", "sets.lua"),
        k("inn", "math", "math", "auto", r"\in ", "sets.lua"),
        k("sub", "math", "math", "auto", r"\subseteq", "sets.lua"),
        k("sup=", "math", "math", "auto", r"\supseteq{}", "sets.lua"),
        k("//", "math", "math", "auto", r"\frac{<>}{<>}", "operations.lua"),
        k("sr", "suffix", "math", "auto", "^{2}", "operations.lua"),
        k("invs", "suffix", "math", "auto", "^{-1}", "operations.lua"),
        k("xx", "math", "math", "auto", r"\times", "symbols.lua"),
        k("sum", "math", "math", "auto", r"\sum", "symbols.lua"),
        k("lim", "math", "math", "auto", r"\lim_{ <:n> \to <:\infty> }", "symbols.lua"),
        k("pi", "math", "math", "auto", r"\pi", "greek.lua"),
        k("par", "math", "math", "tab", r"\frac{ \partial <:y> }{ \partial <:x> } ", "calculus.lua"),
        k("int", "math", "math", "tab", r"\int <> \, d<:x> ", "calculus.lua"),
        k("mk", "text", "text", "auto", "$<>$", "text.lua"),
        k("hat", "post", "math", "auto", r"\hat{%1}", "accents.lua"),
        k("hat", "math", "math", "auto", r"\hat{<>}", "accents.lua"),
        k("B", "visual", "math", "visual", r"\underset{ <> }{ %V }", "visual.lua"),
    ];
    let mut beg = k("beg", "text", "text", "tab", "\\begin{<1>}\n\t<2>\n\\end{<1>}", "text.lua");
    beg.line_rule = Some("start".into());
    beg.ft = Some("tex".into());
    beg.origin = "castel".into();
    v.push(beg);
    let mut sub = k("‹letter›‹digit›", "pattern", "math", "auto", "%1_{%2}", "subscripts.lua");
    sub.pattern = Some("(%a)(%d)".into());
    sub.priority = -1;
    v.push(sub);
    v.iter_mut().filter(|x| x.kind == "post").for_each(|x| x.pattern = Some(format!("(%a){}", x.trigger)));
    v
}

fn codes(fs: &[Finding]) -> Vec<&str> {
    fs.iter().map(|f| f.code.as_str()).collect()
}

fn has(fs: &[Finding], code: &str) -> bool {
    fs.iter().any(|f| f.code == code)
}

fn get<'a>(fs: &'a [Finding], code: &str) -> &'a Finding {
    fs.iter().find(|f| f.code == code).unwrap_or_else(|| panic!("no {code} in {:?}", codes(fs)))
}

// ---------------------------------------------------------------------------
// guess

#[test]
fn guesses() {
    let mut d = draft("D", vec![t(r"\bm{"), Piece::Selection, t("}")]);
    guess(&mut d, None);
    assert_eq!((d.kind, d.context), (Kind::Visual, Context::Math));

    let mut d = draft("bbm", vec![t(r"\mathbb{"), Piece::Capture(1), t("}")]);
    guess(&mut d, Some(Context::Text));
    assert_eq!((d.kind, d.context), (Kind::Post, Context::Math), "a post snippet is maths whatever the place");

    let mut d = draft("(%a)vv(%d)", vec![t(r"\vec{"), Piece::Capture(1), t("}_{"), Piece::Capture(2), t("}")]);
    guess(&mut d, None);
    assert_eq!(d.kind, Kind::Pattern);

    // %1 with a non-letter trigger and no pattern magic: stays plain (check says why)
    let mut d = draft(",.", vec![t(r"\mathbf{"), Piece::Capture(1), t("}")]);
    guess(&mut d, None);
    assert_eq!(d.kind, Kind::Plain);

    let mut d = draft("sec", vec![t(r"\section{"), tab(1), t("}")]);
    guess(&mut d, None);
    assert_eq!((d.context, d.fires, d.line, d.ft), (Context::Text, Fires::Tab, LineRule::Start, FileType::Tex));

    let mut d = draft("ff", parse_body(r"\frac{<>}{<>}"));
    guess(&mut d, Some(Context::Text));
    assert_eq!((d.kind, d.context, d.fires, d.ft), (Kind::Plain, Context::Math, Fires::Auto, FileType::Both));

    // a text trigger that is / starts English words → Tab
    let mut d = draft("the", vec![t(r"\section{"), tab(1), t("}")]);
    guess(&mut d, Some(Context::Text));
    assert_eq!((d.context, d.fires), (Context::Text, Fires::Tab));
    // mk ($…$) is text, and "mk" starts no words: by itself
    let mut d = draft("mk", parse_body("$<>$"));
    guess(&mut d, None);
    assert_eq!((d.context, d.fires, d.line, d.ft), (Context::Text, Fires::Auto, LineRule::Anywhere, FileType::Both));
    // a Castel-style environment: Tab at the start of a line, .tex only
    let mut d = draft("thm", parse_body("\\begin{theorem}\n\t<>\n\\end{theorem}"));
    guess(&mut d, None);
    assert_eq!((d.context, d.fires, d.line, d.ft), (Context::Text, Fires::Tab, LineRule::Start, FileType::Tex));
    // a maths environment stays maths, for .tex and .md
    let mut d = draft("pmat", parse_body("\\begin{pmatrix}\n<>\n\\end{pmatrix}"));
    guess(&mut d, Some(Context::Text));
    assert_eq!((d.context, d.ft), (Context::Math, FileType::Both));
    // display maths delimiters are text, .tex only
    let mut d = draft("dd", parse_body("\\[\n\t<>\n\\]"));
    guess(&mut d, None);
    assert_eq!((d.context, d.ft), (Context::Text, FileType::Tex));
    // nothing to go on: the place decides
    let mut d = draft("hw", vec![t("hello world")]);
    guess(&mut d, Some(Context::Text));
    assert_eq!(d.context, Context::Text);
    guess(&mut d, None);
    assert_eq!(d.context, Context::Math);
    // \text{…} is a maths thing
    let mut d = draft("tx", parse_body(r"\text{<>}"));
    guess(&mut d, Some(Context::Text));
    assert_eq!(d.context, Context::Math);
}

#[test]
fn pinned_fields_stay() {
    let mut d = draft("D", vec![t(r"\bm{"), Piece::Selection, t("}")]);
    d.pinned = vec!["kind".into()];
    guess(&mut d, None);
    assert_eq!(d.kind, Kind::Plain);

    let mut d = draft("sec", vec![t(r"\section{"), tab(1), t("}")]);
    d.pinned = vec!["context".into()];
    guess(&mut d, None);
    assert_eq!(d.context, Context::Math);

    let mut d = with("the", vec![t(r"\theta")], |d| {
        d.context = Context::Text;
        d.fires = Fires::Auto;
        d.line = LineRule::AfterText;
        d.ft = FileType::Markdown;
        d.math_where = MathWhere::Display;
        d.pinned = ["kind", "context", "fires", "math_where", "line", "ft"].map(String::from).to_vec();
    });
    let before = d.clone();
    guess(&mut d, Some(Context::Math));
    assert_eq!(d, before);

    // raw Lua is never re-guessed
    let mut d = Draft { kind: Kind::RawLua, raw_lua: "math(\"a\", [[b]])".into(), ..Draft::default() };
    let before = d.clone();
    guess(&mut d, Some(Context::Text));
    assert_eq!(d, before);
}

// ---------------------------------------------------------------------------
// check: every code at least once

#[test]
fn errors() {
    let kn = known();
    let fs = check(&draft("", vec![t("x")]), &kn);
    assert_eq!(codes(&fs), ["empty-trigger"]);
    assert_eq!(fs[0].severity, Severity::Error);
    assert_eq!(fs[0].message, "Trigger is empty.");

    let fs = check(&draft("zq", vec![]), &kn);
    assert_eq!(get(&fs, "empty-output").severity, Severity::Error);
    assert_eq!(get(&fs, "empty-output").message, "Output is empty.");
    assert!(has(&check(&draft("zq", vec![t("")]), &kn), "empty-output"));

    // a default helpers.parse can't hold
    let d = draft("gtd", vec![tabd(1, "a>b")]);
    let fs = check(&d, &kn);
    let f = get(&fs, "bad-default");
    assert_eq!(f.severity, Severity::Error);
    let fixed = f.fix.as_ref().unwrap().draft.clone().unwrap();
    assert_eq!(fixed.output, vec![tabd(1, "ab")]);
    assert!(!has(&check(&fixed, &kn), "bad-default"));
    assert!(has(&check(&draft("gtd", vec![tabd(1, "a\nb")]), &kn), "bad-default"));

    // text + pattern
    let d = with("(%a)q", vec![Piece::Capture(1)], |d| {
        d.kind = Kind::Pattern;
        d.context = Context::Text;
        d.eg = "xq".into();
    });
    let fs = check(&d, &kn);
    let f = get(&fs, "pattern-text");
    assert_eq!(f.severity, Severity::Error);
    assert_eq!(f.message, "Pattern triggers work in maths only.");
    let fixed = f.fix.as_ref().unwrap().draft.clone().unwrap();
    assert_eq!(fixed.context, Context::Math);
    assert!(fixed.pinned.contains(&"context".to_string()));
    assert!(!check(&fixed, &kn).iter().any(|f| f.severity == Severity::Error));
    // …on Tab too: pat is maths-only in helpers.lua whatever opts.tab says
    let mut tabbed = d.clone();
    tabbed.fires = Fires::Tab;
    assert!(has(&check(&tabbed, &kn), "pattern-text"));

    // not a Lua pattern
    for bad in ["(%a", "%a)", "x%", "[ab", "(%a)%2"] {
        let d = with(bad, vec![t("x")], |d| d.kind = Kind::Pattern);
        let fs = check(&d, &kn);
        assert_eq!(get(&fs, "bad-pattern").severity, Severity::Error, "{bad}");
    }

    // raw Lua that doesn't read
    let d = Draft { kind: Kind::RawLua, raw_lua: "math(\"a\", [[b]], \"yours\"".into(), ..Draft::default() };
    assert_eq!(codes(&check(&d, &kn)), ["lua-error"]);
    let d = Draft { kind: Kind::RawLua, raw_lua: "math(\"a\", \"b, \"yours\")".into(), ..Draft::default() };
    assert_eq!(codes(&check(&d, &kn)), ["lua-error"]);
    let d = Draft { kind: Kind::RawLua, raw_lua: "  ".into(), ..Draft::default() };
    assert_eq!(codes(&check(&d, &kn)), ["empty-output"]);
    let d = Draft { kind: Kind::RawLua, raw_lua: "env(\"pmat\", \"pmatrix\", \"yours\"),".into(), ..Draft::default() };
    assert!(check(&d, &kn).is_empty());

    // a clean draft: nothing at all
    assert!(check(&draft("ff", parse_body(r"\frac{<>}{<>}")), &kn).is_empty());
    assert!(check(&draft("ff", parse_body(r"\frac{<>}{<>}")), &[]).is_empty());
}

#[test]
fn duplicates() {
    let kn = known();
    // same trigger, same place: a warning with a priority fix
    let d = draft("xx", vec![t(r"\chi")]);
    let fs = check(&d, &kn);
    let f = get(&fs, "duplicate");
    assert_eq!(f.severity, Severity::Warning);
    assert_eq!(f.message, "‘xx’ is already a snippet: \\times in symbols.lua (yours).");
    let fix = f.fix.as_ref().unwrap();
    assert_eq!(fix.label, "Use priority 1");
    let fixed = fix.draft.clone().unwrap();
    assert_eq!(fixed.priority, 1);
    // once it outranks hers it is only a note
    let fs = check(&fixed, &kn);
    assert_eq!(get(&fs, "duplicate").severity, Severity::Note);
    assert!(get(&fs, "duplicate").fix.is_none());

    // the body is shown short, tab stops as …
    let fs = check(&draft("//", vec![t(r"\dfrac{"), tab(1), t("}{"), tab(2), t("}")]), &kn);
    assert!(get(&fs, "duplicate").message.contains(r"\frac{…}{…} in operations.lua"), "{}", get(&fs, "duplicate").message);

    // math_tab par = calculus par (both Tab)
    let d = with("par", vec![t(r"\parallel")], |d| d.fires = Fires::Tab);
    assert!(has(&check(&d, &kn), "duplicate"));
    // vis B = visual.lua B
    let d = with("B", vec![t(r"\mathbf{"), Piece::Selection, t("}")], |d| d.kind = Kind::Visual);
    assert_eq!(codes(&check(&d, &kn)), ["duplicate"]);
    // post hat = accents post hat (not the plain hat)
    let d = with("hat", vec![t(r"\widehat{"), Piece::Capture(1), t("}")], |d| d.kind = Kind::Post);
    let fs = check(&d, &kn);
    assert!(get(&fs, "duplicate").message.contains(r"\hat{%1}"));
    // a pattern duplicates by its pattern
    let d = with("(%a)(%d)", vec![Piece::Capture(1), t("_"), Piece::Capture(2)], |d| d.kind = Kind::Pattern);
    assert!(has(&check(&d, &kn), "duplicate"));

    // a different place is no duplicate: text sum vs maths sum (Tab: no words warning either)
    let d = with("sum", vec![t("summary")], |d| {
        d.context = Context::Text;
        d.fires = Fires::Tab;
    });
    assert!(check(&d, &kn).is_empty(), "{:?}", codes(&check(&d, &kn)));
    // inline vs display don't meet; .md vs .tex don't meet
    let mut kn2 = kn.clone();
    kn2.push({
        let mut x = k("nrm", "math", "display", "auto", r"\|<>\|", "brackets.lua");
        x.ft = Some("tex".into());
        x
    });
    let d = with("nrm", vec![t("n")], |d| d.math_where = MathWhere::Inline);
    assert!(!has(&check(&d, &kn2), "duplicate"));
    let d = with("nrm", vec![t("n")], |d| d.ft = FileType::Markdown);
    assert!(!has(&check(&d, &kn2), "duplicate"));
    assert!(has(&check(&draft("nrm", vec![t("n")]), &kn2), "duplicate"));
    // text: line start vs after text don't meet
    let d = with("beg", vec![t("x")], |d| {
        d.context = Context::Text;
        d.fires = Fires::Tab;
        d.line = LineRule::AfterText;
    });
    assert!(!has(&check(&d, &kn), "duplicate"));
    let d = with("beg", vec![t("x")], |d| {
        d.context = Context::Text;
        d.fires = Fires::Tab;
        d.line = LineRule::Start;
    });
    assert!(has(&check(&d, &kn), "duplicate"));

    // the draft's own inbox entry is not a conflict
    let mut kn3 = kn.clone();
    kn3.push(k("qq", "math", "math", "auto", r"\quad", "inbox.lua"));
    let d = draft("qq", vec![t(r"\qquad")]);
    assert!(has(&check(&d, &kn3), "duplicate"), "a new draft does clash with the inbox entry");
    let mut editing = d.clone();
    editing.id = Some("qq-1432".into());
    assert!(!has(&check(&editing, &kn3), "duplicate"));
}

#[test]
fn shadowing() {
    let kn = known();
    // auto `in` fires before inn, int (a Tab one) and invs (a suffix) can be typed
    let fs = check(&draft("in", vec![t(r"\in")]), &kn);
    let f = get(&fs, "shadows");
    assert_eq!(f.severity, Severity::Warning);
    for w in ["‘inn’ (sets.lua)", "‘int’ (calculus.lua)", "‘invs’ (operations.lua)"] {
        assert!(f.message.contains(w), "{}", f.message);
    }
    assert!(f.message.starts_with("‘in’ fires before you can finish"));
    assert_eq!(f.fix.as_ref().unwrap().label, "Make it Tab-only");
    // Tab-only: it no longer fires while typing
    let fixed = f.fix.as_ref().unwrap().draft.clone().unwrap();
    assert!(!has(&check(&fixed, &kn), "shadows"));

    // su shadows sum, sub, sup=
    let f = check(&draft("su", vec![t(r"\sigma_u")]), &kn);
    let m = &get(&f, "shadows").message;
    assert!(m.contains("‘sum’") && m.contains("‘sub’") && m.contains("‘sup=’"), "{m}");
    // text auto `be` shadows the Tab shortcut beg
    let d = with("be", vec![t(r"\beta")], |d| d.context = Context::Text);
    assert!(get(&check(&d, &kn), "shadows").message.contains("‘beg’ (text.lua)"));

    // shadowed: lim2 (lim), srx (the suffix sr), pix (pi), a1b (the subscript pattern)
    for (trig, by) in [("lim2", "‘lim’ (symbols.lua)"), ("srx", "‘sr’ (operations.lua)"), ("pix", "‘pi’ (greek.lua)"), ("a1b", "‘‹letter›‹digit›’ (subscripts.lua)")] {
        let fs = check(&draft(trig, vec![t("x")]), &kn);
        let f = get(&fs, "shadowed-by");
        assert_eq!(f.severity, Severity::Warning);
        assert!(f.message.starts_with(by), "{trig}: {}", f.message);
        assert!(f.message.contains(&format!("so ‘{trig}’ can never be typed")), "{}", f.message);
    }
    assert!(get(&check(&draft("a1b", vec![t("x")]), &kn), "shadowed-by").message.contains("fires at ‘a1’"));
    // a Tab draft is shadowed too (the automatic one fires before Tab)
    let d = with("pix", vec![t("x")], |d| d.fires = Fires::Tab);
    assert!(has(&check(&d, &kn), "shadowed-by"));
    // a letter trigger never fires right after a letter: `uu` inside `xuu` is fine…
    assert!(!has(&check(&draft("xuuz", vec![t("x")]), &kn), "shadowed-by"));
    // …but a suffix does (xsr → x^{2})
    assert!(has(&check(&draft("xsrz", vec![t("x")]), &kn), "shadowed-by"));
    // nothing fires on the way: no finding
    let fs = check(&draft("ff", vec![t("x")]), &kn);
    assert!(!has(&fs, "shadows") && !has(&fs, "shadowed-by"));
    // different context: maths `pi` doesn't touch a text `pix`
    let d = with("pix", vec![t("x")], |d| d.context = Context::Text);
    assert!(!has(&check(&d, &kn), "shadowed-by"));
    // a pattern draft is checked through its example keys
    let d = with("pi(%d)", vec![t(r"\pi_{"), Piece::Capture(1), t("}")], |d| {
        d.kind = Kind::Pattern;
        d.eg = "pi2".into();
    });
    assert!(get(&check(&d, &kn), "shadowed-by").message.starts_with("‘pi’ (greek.lua)"));
    // the same keys, different snippets: LuaSnip takes the higher priority
    // (a tie is load order). A post `um` ties with `sum` when typed as "sum":
    let d = with("um", vec![t(r"\mu{"), Piece::Capture(1), t("}")], |d| d.kind = Kind::Post);
    assert_eq!(get(&check(&d, &kn), "shadows").message, "‘um’ fires instead of ‘sum’ (symbols.lua) on the same keys.");
    // the subscript pattern (priority -1) loses to a plain `x2`, which is fine…
    assert!(!has(&check(&draft("x2", vec![t("x")]), &kn), "shadowed-by"));
    // …but wins over one at priority -2
    let d = with("x2", vec![t("x")], |d| d.priority = -2);
    assert_eq!(
        get(&check(&d, &kn), "shadowed-by").message,
        "‘‹letter›‹digit›’ (subscripts.lua) fires instead when you type ‘x2’, so this one never expands."
    );
}

#[test]
fn trigger_findings() {
    let kn = known();
    // a text auto trigger that starts common words
    let d = with("the", vec![t(r"\theta")], |d| d.context = Context::Text);
    let fs = check(&d, &kn);
    let f = get(&fs, "english-words");
    assert_eq!(f.severity, Severity::Warning);
    assert!(f.message.starts_with("‘the’ starts "), "{}", f.message);
    assert!(f.message.contains("English words (the, there, their, …)"), "{}", f.message);
    let fixed = f.fix.as_ref().unwrap().draft.clone().unwrap();
    assert_eq!(fixed.fires, Fires::Tab);
    assert!(!has(&check(&fixed, &kn), "english-words"));
    // in maths only a trigger that is itself a word, and only as a note
    let fs = check(&draft("and", vec![t(r"\land")]), &kn);
    assert_eq!(get(&fs, "english-words").severity, Severity::Note);
    assert!(!has(&check(&draft("ali", vec![t(r"\aleph")]), &kn), "english-words"));
    // mk starts no words
    let d = with("mk", parse_body("$<>$"), |d| d.context = Context::Text);
    assert!(check(&d, &[]).is_empty());

    // single letters
    let fs = check(&draft("q", vec![t(r"\quad")]), &kn);
    let f = get(&fs, "single-letter");
    assert_eq!(f.severity, Severity::Warning);
    assert_eq!(f.message, "‘q’ is a single letter: it fires every time you type q in maths.");
    assert!(f.fix.is_some());
    assert!(!has(&check(&with("q", vec![t(r"\quad")], |d| d.fires = Fires::Tab), &kn), "single-letter"));
    // a visual key is meant to be one letter
    let d = with("Q", vec![t(r"\sqrt{"), Piece::Selection, t("}")], |d| d.kind = Kind::Visual);
    assert!(check(&d, &kn).is_empty(), "{:?}", codes(&check(&d, &kn)));

    // a backslash trigger fires inside longer commands
    let fs = check(&draft(r"\in", vec![t(r"\in ")]), &[]);
    let f = get(&fs, "trigger-backslash");
    assert_eq!(f.severity, Severity::Warning);
    assert!(f.message.contains(r"(\inf, \infty, \int, …)"), "{}", f.message);
    assert!(!has(&check(&with(r"\int", vec![t(r"\int")], |d| d.fires = Fires::Tab), &[]), "trigger-backslash"));
    assert!(!has(&check(&draft(r"\\\", vec![t(r"\setminus")]), &[]), "trigger-backslash"));
}

#[test]
fn selection_and_captures() {
    let kn = known();
    let d = with("D", vec![t(r"\bm{x}")], |d| d.kind = Kind::Visual);
    let fs = check(&d, &kn);
    let f = get(&fs, "no-selection");
    assert_eq!(f.severity, Severity::Warning);
    assert_eq!(f.fix.as_ref().unwrap().draft.as_ref().unwrap().kind, Kind::Plain);

    let d = with("bbm", vec![t(r"\mathbb{R}")], |d| d.kind = Kind::Post);
    let fs = check(&d, &kn);
    let f = get(&fs, "no-capture");
    assert_eq!(f.severity, Severity::Warning);
    assert_eq!(f.fix.as_ref().unwrap().draft.as_ref().unwrap().kind, Kind::Suffix);
    // %2 in a post, %3 in a two-capture pattern
    let d = with("bbm", vec![Piece::Capture(1), Piece::Capture(2)], |d| d.kind = Kind::Post);
    assert!(get(&check(&d, &kn), "no-capture").message.contains("%2"));
    let d = with("(%a)q(%d)", vec![Piece::Capture(3)], |d| {
        d.kind = Kind::Pattern;
        d.eg = "xq2".into();
    });
    assert!(get(&check(&d, &kn), "no-capture").message.contains("captures only 2"));

    // %V outside a visual snippet, %1 outside post/pattern
    let fs = check(&draft("sq", vec![t(r"\sqrt{"), Piece::Selection, t("}")]), &[]);
    assert_eq!(get(&fs, "selection-unused").fix.as_ref().unwrap().draft.as_ref().unwrap().kind, Kind::Visual);
    let fs = check(&draft("bbm", vec![t(r"\mathbb{"), Piece::Capture(1), t("}")]), &[]);
    assert_eq!(get(&fs, "capture-unused").fix.as_ref().unwrap().draft.as_ref().unwrap().kind, Kind::Post);
    let fs = check(&draft(",.", vec![t(r"\mathbf{"), Piece::Capture(1), t("}")]), &[]);
    assert!(get(&fs, "capture-unused").fix.is_none());
}

#[test]
fn notes_and_order() {
    let kn = known();
    let fs = check(&draft("ltg", vec![t("a<>b")]), &kn);
    let f = get(&fs, "literal-angle");
    assert_eq!(f.severity, Severity::Note);
    assert_eq!(f.message, "‘<>’ in the output is literal text; it is written %<%> so it isn't read as a tab stop.");
    assert!(!fs.iter().any(|f| f.severity == Severity::Error));
    assert!(get(&check(&draft("lt3", vec![t("x<3>y")]), &[]), "literal-angle").message.contains("%<3%>"));
    assert!(!has(&check(&draft("ltb", vec![t("a<b>c")]), &[]), "literal-angle"));

    let d = draft("mrd", vec![tab(1), t(" = "), tabd(1, "x")]);
    let fs = check(&d, &[]);
    let f = get(&fs, "mirror-default");
    assert_eq!(f.severity, Severity::Note);
    assert_eq!(f.fix.as_ref().unwrap().draft.as_ref().unwrap().output, vec![tabd(1, "x"), t(" = "), tab(1)]);
    assert!(!has(&check(&draft("mrd", vec![tabd(1, "x"), t(" = "), tab(1)]), &[]), "mirror-default"));

    let fs = check(&draft("nlz", vec![t("\\quad\n")]), &[]);
    assert_eq!(get(&fs, "trailing-newline").severity, Severity::Note);
    assert!(!has(&check(&draft("nlz", vec![t("\\quad\n"), Piece::Exit]), &[]), "trailing-newline"));

    let d = with("(%a)vv(%d)", vec![t(r"\vec{"), Piece::Capture(1), t("}_{"), Piece::Capture(2), t("}")], |d| d.kind = Kind::Pattern);
    let fs = check(&d, &[]);
    let f = get(&fs, "pattern-eg");
    assert_eq!(f.severity, Severity::Note);
    assert_eq!(f.fix.as_ref().unwrap().draft.as_ref().unwrap().eg, "xvv2");

    let d = with("aa", vec![t(r"\alpha")], |d| d.check = r#"return line:find("=", 1, true) ~= nil"#.into());
    assert_eq!(get(&check(&d, &[]), "lua-check").severity, Severity::Note);

    // most severe first
    let d = with("q", vec![tab(1), t(" <> "), tabd(1, "x>")], |d| d.check = "return true".into());
    let fs = check(&d, &kn);
    let sev: Vec<Severity> = fs.iter().map(|f| f.severity).collect();
    let mut sorted = sev.clone();
    sorted.sort_by_key(|s| match s {
        Severity::Error => 0,
        Severity::Warning => 1,
        Severity::Note => 2,
    });
    assert_eq!(sev, sorted);
    assert_eq!(fs[0].code, "bad-default");
    assert!(has(&fs, "single-letter") && has(&fs, "literal-angle") && has(&fs, "lua-check"));
    // every message is one sentence, no exclamation marks
    for f in &fs {
        assert!(f.message.ends_with('.') && !f.message.contains('!') && !f.message.contains('\n'), "{}", f.message);
    }
}

#[test]
fn every_code_is_covered() {
    // the stable codes, as documented on infer::check
    let kn = known();
    let mut seen: Vec<String> = Vec::new();
    let mut add = |fs: Vec<Finding>| seen.extend(fs.into_iter().map(|f| f.code));
    add(check(&draft("", vec![t("x")]), &kn));
    add(check(&draft("zq", vec![]), &kn));
    add(check(&draft("gtd", vec![tabd(1, "a>b")]), &kn));
    add(check(&with("(%a", vec![t("x")], |d| {
        d.kind = Kind::Pattern;
        d.context = Context::Text;
    }), &kn));
    add(check(&Draft { kind: Kind::RawLua, raw_lua: "math(".into(), ..Draft::default() }, &kn));
    add(check(&draft("xx", vec![t("x")]), &kn));
    add(check(&draft("in", vec![t("x")]), &kn));
    add(check(&draft("pix", vec![t("x")]), &kn));
    add(check(&with("the", vec![t("x")], |d| d.context = Context::Text), &kn));
    add(check(&draft("q", vec![t("x")]), &kn));
    add(check(&draft(r"\in", vec![t("x")]), &kn));
    add(check(&with("D", vec![t("x")], |d| d.kind = Kind::Visual), &kn));
    add(check(&with("bbm", vec![t("x")], |d| d.kind = Kind::Post), &kn));
    add(check(&draft("zs", vec![Piece::Selection, Piece::Capture(1)]), &kn));
    add(check(&draft("zl", vec![t("<>"), tab(1), tabd(1, "y"), t("\n")]), &kn));
    add(check(&with("(%a)zz", vec![Piece::Capture(1)], |d| {
        d.kind = Kind::Pattern;
        d.check = "return true".into();
    }), &kn));
    for code in [
        "empty-trigger", "empty-output", "bad-default", "pattern-text", "bad-pattern", "lua-error", "duplicate", "shadows",
        "shadowed-by", "english-words", "single-letter", "trigger-backslash", "no-selection", "no-capture", "selection-unused",
        "capture-unused", "literal-angle", "mirror-default", "trailing-newline", "pattern-eg", "lua-check",
    ] {
        assert!(seen.iter().any(|s| s == code), "{code} never came up");
    }
}

// ---------------------------------------------------------------------------
// try plan

#[test]
fn try_plans() {
    let p = try_plan(&draft("ff", parse_body(r"\frac{<>}{<>}")));
    assert_eq!((p.ft.as_str(), p.place.as_str(), p.keys.as_str(), p.tab), ("tex", "inline", "ff", false));
    assert_eq!(p.fills, ["a", "b"]);
    assert_eq!(p.entry, "");

    let p = try_plan(&with("ip", vec![t("<"), tab(1), t(">")], |d| d.fires = Fires::Tab));
    assert_eq!((p.place.as_str(), p.keys.as_str(), p.tab, p.fills.len()), ("inline", "ip", true, 1));
    // mirrors are one tab stop; the exit isn't one
    let p = try_plan(&draft("bnv", vec![t(r"\begin{"), tab(1), t("}"), tab(2), t(r"\end{"), tab(1), t("}"), Piece::Exit]));
    assert_eq!(p.fills, ["a", "b"]);

    let p = try_plan(&with("bbm", vec![t(r"\mathbb{"), Piece::Capture(1), t("}")], |d| d.kind = Kind::Post));
    assert_eq!((p.keys.as_str(), p.tab), ("xbbm", false));
    let p = try_plan(&with("tp", vec![t(r"^{\top}")], |d| d.kind = Kind::Suffix));
    assert_eq!(p.keys, "xtp");
    let p = try_plan(&with("(%a)vv(%d)", vec![Piece::Capture(1)], |d| {
        d.kind = Kind::Pattern;
        d.eg = "xvv2".into();
    }));
    assert_eq!(p.keys, "xvv2");
    let p = try_plan(&with("pa(%a)(%a)", vec![Piece::Capture(1)], |d| {
        d.kind = Kind::Pattern;
        d.fires = Fires::Tab;
    }));
    assert_eq!((p.keys.as_str(), p.tab, p.place.as_str()), ("paxy", true, "inline"));
    let p = try_plan(&with("D", vec![t(r"\bm{"), Piece::Selection, t("}")], |d| d.kind = Kind::Visual));
    assert_eq!((p.place.as_str(), p.keys.as_str(), p.tab), ("visual", "D", false));
    let p = try_plan(&with("thm", vec![t("x")], |d| {
        d.context = Context::Text;
        d.fires = Fires::Tab;
        d.line = LineRule::Start;
    }));
    assert_eq!((p.place.as_str(), p.tab), ("text-start", true));
    let p = try_plan(&with("todo", vec![t("x")], |d| {
        d.context = Context::Text;
        d.line = LineRule::ListItem;
    }));
    assert_eq!(p.place, "list-item");
    let p = try_plan(&with("tt", vec![t("x")], |d| d.context = Context::Text));
    assert_eq!(p.place, "text");
    let p = try_plan(&with("nrm", vec![t("x")], |d| d.math_where = MathWhere::Display));
    assert_eq!(p.place, "display");
    let p = try_plan(&with("mdq", vec![t("x")], |d| d.ft = FileType::Markdown));
    assert_eq!(p.ft, "markdown");
    let p = try_plan(&with("sect", vec![t("x")], |d| {
        d.context = Context::Text;
        d.fires = Fires::Tab;
        d.line = LineRule::Start;
        d.ft = FileType::Tex;
    }));
    assert_eq!((p.ft.as_str(), p.place.as_str()), ("tex", "text-start"));
    // raw Lua that reads as a helper call plans as that call
    let p = try_plan(&Draft { kind: Kind::RawLua, raw_lua: r#"math_tab("ip", [[\langle <>, <> \rangle]], "yours"),"#.into(), ..Draft::default() });
    assert_eq!((p.keys.as_str(), p.tab, p.fills.len()), ("ip", true, 2));
    let p = try_plan(&Draft { kind: Kind::RawLua, raw_lua: r#"env("pmat", "pmatrix", "yours"),"#.into(), ..Draft::default() });
    assert_eq!(p.keys, "pmat");
}

// ---------------------------------------------------------------------------
// Lua patterns

#[test]
fn lua_patterns() {
    let caps = |s: &str, p: &str| lua_find(s, p).unwrap().map(|m| m.2);
    assert_eq!(caps("x2", "(%a)(%d)$"), Some(vec!["x".into(), "2".into()]));
    assert_eq!(caps("a x2", "(%a)(%d)$"), Some(vec!["x".into(), "2".into()]));
    assert_eq!(caps(r"$\alpha2", r"\(%a+)(%d)$"), Some(vec!["alpha".into(), "2".into()]));
    assert_eq!(caps(r"\sinx", r"\(%a+)(%a)$"), Some(vec!["sin".into(), "x".into()]));
    assert_eq!(caps("iden3", "iden(%d)$"), Some(vec!["3".into()]));
    assert_eq!(caps("x_{12}3", "(%a)_{(%d+)}(%d)$"), Some(vec!["x".into(), "12".into(), "3".into()]));
    assert_eq!(caps("xhat", "(%a)hat$"), Some(vec!["x".into()]));
    assert_eq!(caps("hat", "(%a)hat$"), None);
    assert_eq!(caps("a.b", "%."), Some(vec![".".into()]));
    assert_eq!(caps("ab_c9", "^[%a_]+"), Some(vec!["ab_c".into()]));
    assert_eq!(caps("zab", "^ab"), None);
    assert_eq!(caps("f(a(b)c)", "%b()"), Some(vec!["(a(b)c)".into()]));
    assert_eq!(caps("THE (quick) fox", "%f[%a]%a+"), Some(vec!["THE".into()]));
    assert_eq!(caps("aaab", "a-b"), Some(vec!["aaab".into()]));
    assert_eq!(caps("xyxy", "(xy)%1"), Some(vec!["xy".into()]));
    assert_eq!(caps("q", "[^%d]"), Some(vec!["q".into()]));
    assert_eq!(caps("5", "[^%d]"), None);
    assert_eq!(caps("ab", "()b"), Some(vec!["2".into()]));
    assert_eq!(lua_find("abc", "b").unwrap().map(|m| (m.0, m.1)), Some((1, 2)));
    assert!(lua_find("x", "(%a").is_err());
    for bad in ["(", ")", "%", "[a", "%1", "(a)%2", "%b(", "%fa"] {
        assert!(lua_pattern_check(bad).is_err(), "{bad}");
    }
    assert_eq!(lua_pattern_check(r"\(%a+){\(%a+){(\?%a+)}}(%d)"), Ok(4));
    assert_eq!(infer::lua_pesc("(x)%.+"), "%(x%)%%%.%+");

    assert_eq!(pattern_example("pa(%a)(%a)"), "paxy");
    assert_eq!(pattern_example("(%a)(%d)"), "x2");
    assert_eq!(pattern_example("iden(%d)"), "iden2");
    assert_eq!(pattern_example(r"\(%a+)(%d)"), r"\x2");
    assert_eq!(pattern_example(r"\(%a+),%."), r"\x,.");
    assert_eq!(pattern_example("([%a_])q"), "xq");
    assert!(infer::has_pattern_magic("(%a)q") && infer::has_pattern_magic(",%.") && !infer::has_pattern_magic("50%") && !infer::has_pattern_magic("lr("));
}

// ---------------------------------------------------------------------------
// words (queried, never printed)

#[test]
fn words_queries() {
    assert!(words::is_word("the"));
    assert!(words::is_word("offer"));
    assert!(!words::is_word("ff"));
    assert!(!words::is_word(""));
    // most common first: the built-in list's order, then shortest
    let w = words::starting_with("of", 5);
    assert_eq!(w.len(), 5);
    assert_eq!(w[0], "offer");
    assert!(w.iter().all(|x| x.starts_with("of") && x.len() > 2));
    let w = words::starting_with("th", 4);
    assert_eq!(w[..3], ["the", "that", "this"]);
    let w = words::starting_with("ma", 2);
    assert_eq!(w, ["many", "make"]);
    // rest: shorter first, then alphabetical
    let w = words::starting_with("zyg", 50);
    for pair in w.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        let ra = words::common_rank(a).unwrap_or(usize::MAX);
        let rb = words::common_rank(b).unwrap_or(usize::MAX);
        assert!((ra, a.len(), a) <= (rb, b.len(), b), "{a} before {b}");
    }
    // case-sensitive, as triggers are
    assert!(words::starting_with("The", 3).iter().all(|x| x.starts_with("The")));
    assert!(words::starting_with("ff", 10).is_empty());
    assert!(words::starting_with("", 10).is_empty());
    assert_eq!(words::starting_with("the", 0).len(), 0);
    assert!(words::count_starting_with("beg") >= words::starting_with("beg", 3).len());
    assert!(words::starts_common_words("beg") && words::starts_common_words("the"));
    assert!(!words::starts_common_words("mk") && !words::starts_common_words("dm") && !words::starts_common_words("ff"));
    words::preload();
}
