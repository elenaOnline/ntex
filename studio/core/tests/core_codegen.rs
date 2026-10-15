//! codegen, luaparse and inbox: Draft → exact Lua, the body round trip, every
//! entry of her real snippet files (read-only), and the inbox file on disk
//! (temp dirs only).
use ntex_studio_core::codegen::{self, body, entry, lua_string, parse_body, renumber};
use ntex_studio_core::model::*;
use ntex_studio_core::{inbox, luaparse};
use std::path::PathBuf;

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

// ---------------------------------------------------------------------------
// Draft → exact Lua (ARCHITECTURE.md "The Lua the studio writes")

#[test]
fn entries_exact() {
    let cases: Vec<(&str, Draft, &str)> = vec![
        ("plain maths", draft("ff", vec![t(r"\frac{"), tab(1), t("}{"), tab(2), t("}")]), r#"  math("ff", [[\frac{<>}{<>}]], "yours"),"#),
        ("]] in the body", draft("dbr", vec![t("a]]b")]), r#"  math("dbr", [=[a]]b]=], "yours"),"#),
        ("]] and ]=] in the body", draft("lv2", vec![t("a]]b]=]c")]), r#"  math("lv2", [==[a]]b]=]c]==], "yours"),"#),
        (
            "a body ending in ]",
            draft("lrs", vec![t(r"\left[ "), tab(1), t(r" \right]")]),
            r#"  math("lrs", [=[\left[ <> \right]]=], "yours"),"#,
        ),
        ("a leading newline", draft("nlx", vec![t("\n\\quad")]), "  math(\"nlx\", [[\n\n\\quad]], \"yours\"),"),
        ("a trailing space", draft("tsp", vec![t(r"\otimes ")]), r#"  math("tsp", [[\otimes ]], "yours"),"#),
        (
            "a tab character, text on Tab at a line start",
            with("thm", vec![t("\\begin{theorem}\n\t"), tab(1), t("\n\\end{theorem}")], |d| {
                d.context = Context::Text;
                d.fires = Fires::Tab;
                d.line = LineRule::Start;
            }),
            "  text(\"thm\", [[\n\\begin{theorem}\n\t<>\n\\end{theorem}]], \"yours\", { tab = true, line = \"start\" }),",
        ),
        ("a literal %", draft("pct", vec![t(r"50\%")]), r#"  math("pct", [[50\%%]], "yours"),"#),
        ("literal %1 and %V", draft("pcx", vec![t(r"\%1 \%V")]), r#"  math("pcx", [[\%%1 \%%V]], "yours"),"#),
        ("literal <>", draft("ltg", vec![t("a<>b")]), r#"  math("ltg", [[a%<%>b]], "yours"),"#),
        ("literal <3>", draft("lt3", vec![t("x<3>y")]), r#"  math("lt3", [[x%<3%>y]], "yours"),"#),
        ("literal <:x>", draft("ltx", vec![t("u<:x>v")]), r#"  math("ltx", [[u%<:x%>v]], "yours"),"#),
        ("<b> is no tab stop", draft("ltb", vec![t("a<b>c")]), r#"  math("ltb", [[a<b>c]], "yours"),"#),
        (
            "angle brackets around a tab stop",
            with("ip", vec![t("<"), tab(1), t(">")], |d| d.fires = Fires::Tab),
            r#"  math_tab("ip", [[<<>>]], "yours"),"#,
        ),
        (
            "a mirror",
            draft("bnv", vec![t(r"\begin{"), tab(1), t("}\n"), tab(2), t("\n\\end{"), tab(1), t("}")]),
            "  math(\"bnv\", [[\n\\begin{<1>}\n<2>\n\\end{<1>}]], \"yours\"),",
        ),
        (
            "defaults",
            draft("dsp", vec![t(r"\sum_{"), tabd(1, "i = 1"), t("}^{"), tabd(2, "n"), t("}")]),
            r#"  math("dsp", [[\sum_{<:i = 1>}^{<:n>}]], "yours"),"#,
        ),
        (
            "a mirrored default",
            draft("mrd", vec![tabd(1, "x"), t(" = "), tab(1)]),
            r#"  math("mrd", [[<1:x> = <1>]], "yours"),"#,
        ),
        ("out-of-order tab stops", draft("frr", vec![t(r"\frac{"), tab(2), t("}{"), tab(1), t("}")]), r#"  math("frr", [[\frac{<2>}{<1>}]], "yours"),"#),
        (
            "the exit point",
            draft("itd", vec![t(r"\int "), Piece::Exit, t(r" \, d"), tabd(1, "x")]),
            r#"  math("itd", [[\int <0> \, d<:x>]], "yours"),"#,
        ),
        (
            "post with %1",
            with("bbm", vec![t(r"\mathbb{"), Piece::Capture(1), t("}")], |d| d.kind = Kind::Post),
            r#"  post("bbm", [[\mathbb{%1}]], "yours"),"#,
        ),
        (
            "a pattern with an example",
            with("(%a)vv(%d)", vec![t(r"\vec{"), Piece::Capture(1), t("}_{"), Piece::Capture(2), t("}")], |d| {
                d.kind = Kind::Pattern;
                d.eg = "xvv2".into();
            }),
            r#"  pat("(%a)vv(%d)", [[\vec{%1}_{%2}]], "yours", { eg = "xvv2" }),"#,
        ),
        (
            "a pattern on Tab",
            with("ve(%a)", vec![t(r"\vec{"), Piece::Capture(1), t("}")], |d| {
                d.kind = Kind::Pattern;
                d.fires = Fires::Tab;
                d.eg = "vex".into();
            }),
            r#"  pat("ve(%a)", [[\vec{%1}]], "yours", { tab = true, eg = "vex" }),"#,
        ),
        (
            "a pattern with a backslash",
            with(r"\(%a+),%.", vec![t(r"\boldsymbol{\"), Piece::Capture(1), t("}")], |d| d.kind = Kind::Pattern),
            r#"  pat([[\(%a+),%.]], [[\boldsymbol{\%1}]], "yours"),"#,
        ),
        (
            "visual with %V",
            with("D", vec![t(r"\boldsymbol{"), Piece::Selection, t("}")], |d| d.kind = Kind::Visual),
            r#"  vis("D", [[\boldsymbol{%V}]], "yours"),"#,
        ),
        ("visual ending in ]", with("[", vec![t("["), Piece::Selection, t("]")], |d| d.kind = Kind::Visual), r#"  vis("[", "[%V]", "yours"),"#),
        (
            "text Tab at a line start, .tex only",
            with("sect", vec![t(r"\section{"), tab(1), t("}")], |d| {
                d.context = Context::Text;
                d.fires = Fires::Tab;
                d.line = LineRule::Start;
                d.ft = FileType::Tex;
            }),
            r#"  text("sect", [[\section{<>}]], "yours", { tab = true, line = "start", ft = "tex" }),"#,
        ),
        (
            "text in a list item",
            with("todo", vec![t(r"\textbf{TODO}: ")], |d| {
                d.context = Context::Text;
                d.line = LineRule::ListItem;
            }),
            r#"  text("todo", [[\textbf{TODO}: ]], "yours", { line = "list item" }),"#,
        ),
        (
            "text after text, on Tab",
            with("eqq", vec![t(r" \eqref{"), tab(1), t("}")], |d| {
                d.context = Context::Text;
                d.fires = Fires::Tab;
                d.line = LineRule::AfterText;
            }),
            r#"  text("eqq", [[ \eqref{<>}]], "yours", { tab = true, line = "after text" }),"#,
        ),
        ("priority", with("sq", vec![t(r"\sqrt[3]{"), tab(1), t("}")], |d| d.priority = 1), r#"  math("sq", [[\sqrt[3]{<>}]], "yours", { priority = 1 }),"#),
        ("negative priority", with("nn", vec![t(r"\cap")], |d| d.priority = -1), r#"  math("nn", [[\cap]], "yours", { priority = -1 }),"#),
        (
            "display only, priority, a desc with quotes and backslashes",
            with("nrm", vec![t(r"\|"), tab(1), t(r"\|")], |d| {
                d.math_where = MathWhere::Display;
                d.priority = 2;
                d.desc = r#"The "norm" \| x \|"#.into();
            }),
            r#"  math("nrm", [[\|<>\|]], "yours", { where = "display", priority = 2, desc = "The \"norm\" \\| x \\|" }),"#,
        ),
        ("inline only", with("hlf", vec![t(r"\tfrac12")], |d| d.math_where = MathWhere::Inline), r#"  math("hlf", [[\tfrac12]], "yours", { where = "inline" }),"#),
        (
            "a check function",
            with("aa", vec![t(r"\alpha")], |d| d.check = r#"return line:find("=", 1, true) ~= nil"#.into()),
            r#"  math("aa", [[\alpha]], "yours", { check = function(line, trigger, captures) return line:find("=", 1, true) ~= nil end }),"#,
        ),
        ("a quote and a backslash in the trigger", draft("\\q\"", vec![t(r"\quad")]), r#"  math([[\q"]], [[\quad]], "yours"),"#),
        ("a double quote trigger", draft("\"", vec![t(r"\text{ "), tab(1), t(" }")]), r#"  math('"', [[\text{ <> }]], "yours"),"#),
        ("backslashes in the trigger", draft(r"\\\", vec![t(r"\setminus{}")]), r#"  math([[\\\]], [[\setminus{}]], "yours"),"#),
        ("both quotes in the trigger", draft("a'\"", vec![t("x")]), r#"  math("a'\"", [[x]], "yours"),"#),
        ("unicode", draft("→", vec![t(r"\to ↦ α")]), r#"  math("→", [[\to ↦ α]], "yours"),"#),
        ("suffix", with("tp", vec![t(r"^{\top}")], |d| d.kind = Kind::Suffix), r#"  suffix("tp", [[^{\top}]], "yours"),"#),
        ("markdown only", with("mdq", vec![t("x")], |d| d.ft = FileType::Markdown), r#"  math("mdq", [[x]], "yours", { ft = "markdown" }),"#),
        (
            "a trailing line break gets an exit point",
            with("hrl", vec![t("\\hrule\n")], |d| {
                d.context = Context::Text;
                d.fires = Fires::Tab;
                d.line = LineRule::Start;
            }),
            "  text(\"hrl\", [[\n\\hrule\n<0>]], \"yours\", { tab = true, line = \"start\" }),",
        ),
        ("a carriage return needs a quoted string", draft("crl", vec![t("a\r\nb")]), r#"  math("crl", "a\r\nb", "yours"),"#),
        ("an empty origin is yours", with("ee", vec![t("e^{"), tab(1), t("}")], |d| d.origin.clear()), r#"  math("ee", [[e^{<>}]], "yours"),"#),
        ("another origin", with("ee", vec![t("e")], |d| d.origin = "castel".into()), r#"  math("ee", [[e]], "castel"),"#),
        (
            "numbered stops keep <> where her files do (bigfun)",
            draft("bf", vec![tab(1), t(": "), tab(2), t(" & "), tab(3), t(r" \\ "), tab(4), t(" & "), tab(1), t("("), tab(4), t(") = "), tab(5)]),
            r#"  math("bf", [[<1>: <2> & <3> \\ <4> & <1>(<4>) = <>]], "yours"),"#,
        ),
        (
            "and a leading one (plot)",
            draft("pl", vec![t("["), tab(1), t("] "), tabd(2, "-10"), t(":"), tab(2)]),
            r#"  math("pl", [[[<>] <2:-10>:<2>]], "yours"),"#,
        ),
        (
            "raw Lua, verbatim with its comma",
            Draft { kind: Kind::RawLua, raw_lua: "  env(\"pmat\", \"pmatrix\", \"yours\")\n".into(), ..Draft::default() },
            r#"  env("pmat", "pmatrix", "yours"),"#,
        ),
        (
            "raw Lua with two calls is one list item",
            Draft { kind: Kind::RawLua, raw_lua: "math(\"a\", [[b]], \"yours\"),\nmath(\"c\", [[d]], \"yours\"),".into(), ..Draft::default() },
            "  { math(\"a\", [[b]], \"yours\"),\nmath(\"c\", [[d]], \"yours\") },",
        ),
    ];
    let n = cases.len();
    let mut bad = Vec::new();
    for (why, d, want) in &cases {
        let got = entry(d);
        if got != *want {
            bad.push(format!("{why}:\n  want {want}\n  got  {got}"));
        }
    }
    assert!(bad.is_empty(), "{} of {n} entries differ:\n{}", bad.len(), bad.join("\n"));
    assert!(n >= 30);
}

/// Every entry above reads back (luaparse) as the draft that wrote it, and
/// the Lua literal of every body holds exactly that body.
#[test]
fn entries_read_back() {
    let drafts = vec![
        draft("ff", vec![t(r"\frac{"), tab(1), t("}{"), tab(2), t("}")]),
        draft("bnv", vec![t(r"\begin{"), tab(1), t("}\n"), tab(2), t("\n\\end{"), tab(1), t("}")]),
        draft("ltx", vec![t("u<:x>v a<>b <3>")]),
        draft("pcx", vec![t(r"50\% \%1 \%V")]),
        draft("nlx", vec![t("\n\\quad")]),
        with("hrl", vec![t("\\hrule\n")], |d| {
            d.context = Context::Text;
            d.fires = Fires::Tab;
            d.line = LineRule::Start;
            d.ft = FileType::Tex;
        }),
        with("(%a)vv(%d)", vec![t(r"\vec{"), Piece::Capture(1), t("}_{"), Piece::Capture(2), t("}")], |d| {
            d.kind = Kind::Pattern;
            d.eg = "xvv2".into();
            d.priority = -1;
        }),
        with("[", vec![t("["), Piece::Selection, t("]")], |d| d.kind = Kind::Visual),
        with("aa", vec![t(r"\alpha")], |d| {
            d.check = "return true".into();
            d.desc = "a \"q\" \\ d".into();
            d.math_where = MathWhere::Inline;
        }),
        draft("\\q\"", vec![t(r"\quad")]),
        draft("a]]b", vec![t("x]=]y]]")]),
        draft("bf", vec![tab(1), t(": "), tab(2), t(" & "), tab(1), t(" = "), tab(3)]),
    ];
    for d in drafts {
        let e = entry(&d);
        let back = luaparse::parse_entry(&e).unwrap_or_else(|| panic!("no draft from {e}"));
        let mut want = d.clone();
        want.pinned = back.pinned.clone();
        assert_eq!(back, want, "{e}");
        assert_eq!(luaparse::trigger_of(&e).as_deref(), Some(d.trigger.as_str()), "{e}");
    }
}

// ---------------------------------------------------------------------------
// round trip: parse_body(body(p)) == p (after renumber), over generated pieces

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn pick<'a>(&mut self, xs: &[&'a str]) -> &'a str {
        xs[self.below(xs.len() as u64) as usize]
    }
}

const BITS: &[&str] = &[
    "a", "x", " ", "\\frac{", "}", "{", "^", "_", "<", ">", "<>", "<3>", "<:y>", "<1:z>", "<0>", "%", "%%", "%1", "%V", "%<", "%>",
    "\n", "\t", "\\left<", "\\right>", "]]", "]", "[", "[[", "]=]", "$", "$1", "${1:x}", ":", "1", "\\", "\\\\", "é", "→", "\"", "'",
];
const DEFAULTS: &[&str] = &["", "", "", "x", "0", "\\infty", "a b", "50%", "f(x)", "%V", "1", "]]", "$1"];

fn merged(v: Vec<Piece>) -> Vec<Piece> {
    let mut out: Vec<Piece> = Vec::new();
    for p in v {
        match (p, out.last_mut()) {
            (Piece::Text(s), _) if s.is_empty() => {}
            (Piece::Text(s), Some(Piece::Text(prev))) => prev.push_str(&s),
            (p, _) => out.push(p),
        }
    }
    renumber(&mut out);
    out
}

fn gen_pieces(r: &mut Rng) -> Vec<Piece> {
    let n = r.below(10);
    let mut v = Vec::new();
    let mut stops = 0u32;
    let mut exit = false;
    for _ in 0..n {
        match r.below(9) {
            0..=2 => {
                let k = 1 + r.below(5);
                v.push(Piece::Text((0..k).map(|_| r.pick(BITS)).collect()));
            }
            3 | 4 => {
                stops += 1;
                v.push(Piece::Tab { n: stops, default: r.pick(DEFAULTS).to_string() });
            }
            5 if stops > 0 => v.push(Piece::Tab { n: 1 + r.below(stops as u64) as u32, default: String::new() }),
            6 if !exit => {
                exit = true;
                v.push(Piece::Exit);
            }
            7 => v.push(Piece::Selection),
            8 => v.push(Piece::Capture(1 + r.below(9) as u8)),
            _ => v.push(t("y")),
        }
    }
    merged(v)
}

#[test]
fn body_round_trip_generated() {
    let mut r = Rng(0x2545_F491_4F6C_DD1D);
    let mut checked = 0;
    for _ in 0..600 {
        let p = gen_pieces(&mut r);
        let b = body(&p);
        // what the Lua file holds reads back as the same pieces
        assert_eq!(merged(codegen::parse_helper_body(&b)), p, "body {b:?}");
        // paste reads helper syntax first; `$1`-looking text without helper
        // tokens is VS Code syntax by design, so only check the others
        if !b.contains('$') {
            assert_eq!(merged(parse_body(&b)), p, "paste {b:?}");
        }
        // the literal holds the body exactly
        let lit = lua_string(&b);
        assert_eq!(luaparse::lua_string_value(&lit).as_deref(), Some(b.as_str()), "{lit}");
        // and the whole entry reads back (when there is something to write)
        let d = draft("zq", p.clone());
        let mut want = d.output.clone();
        // an exit point right after a closing line break is the one codegen
        // writes anyway (codegen::needs_exit), so it reads back as no exit
        if matches!(want.last(), Some(Piece::Exit)) && codegen::needs_exit(&want[..want.len() - 1]) {
            want.pop();
        }
        if let Some(back) = luaparse::parse_entry(&entry(&d)) {
            assert_eq!(merged(back.output), merged(want), "entry {}", entry(&d));
        } else {
            panic!("entry did not read back: {}", entry(&d));
        }
        checked += 1;
    }
    assert_eq!(checked, 600);
}

// ---------------------------------------------------------------------------
// luaparse on her real files (read-only)

/// A call's text with string literals replaced by their values and
/// whitespace dropped outside them: "the same call, modulo spelling".
fn normal_form(code: &str) -> String {
    let b = code.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'"' || c == b'\'' {
            let mut j = i + 1;
            while j < b.len() && b[j] != c {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            let lit = &code[i..=j.min(b.len() - 1)];
            out.push_str(&format!("S{:?}", luaparse::lua_string_value(lit)));
            i = j + 1;
            continue;
        }
        if c == b'[' {
            let mut j = i + 1;
            while j < b.len() && b[j] == b'=' {
                j += 1;
            }
            if j < b.len() && b[j] == b'[' {
                let close = format!("]{}]", "=".repeat(j - i - 1));
                if let Some(k) = code[j..].find(&close) {
                    let end = j + k + close.len();
                    out.push_str(&format!("S{:?}", luaparse::lua_string_value(&code[i..end])));
                    i = end;
                    continue;
                }
            }
        }
        if !c.is_ascii_whitespace() {
            out.push(c as char);
        }
        i += 1;
    }
    out
}

fn squash(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn her_snippet_files() {
    let dir = PathBuf::from(std::env::var("HOME").unwrap()).join(".config/texnvim/snippets/tex");
    let Ok(rd) = std::fs::read_dir(&dir) else {
        eprintln!("no snippets folder; skipped");
        return;
    };
    let mut files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "lua")).collect();
    files.sort();
    let (mut total, mut drafts, mut identical, mut same_meaning) = (0, 0, 0, 0);
    let mut spelled = Vec::new();
    let mut nones = Vec::new();
    for f in &files {
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        if name == "inbox.lua" || name.starts_with("zz_") {
            continue;
        }
        let text = std::fs::read_to_string(f).unwrap();
        let entries = luaparse::split_entries(&text);
        assert!(!entries.is_empty(), "{name}: no entries");
        for (line, src) in entries {
            total += 1;
            let call = luaparse::call_text(&src).unwrap();
            match luaparse::parse_entry(&src) {
                Some(d) => {
                    drafts += 1;
                    let regen = entry(&d);
                    let regen_call = luaparse::call_text(&regen).unwrap();
                    if squash(&regen_call) == squash(&call) {
                        identical += 1;
                    } else {
                        // never a wrong draft: at worst the same call spelled
                        // differently (a "…" string where codegen writes [[…]])
                        assert_eq!(normal_form(&regen_call), normal_form(&call), "{name}:{line}: regenerated differently\n  was {call}\n  now {regen_call}");
                        same_meaning += 1;
                        spelled.push(format!("{name}:{line}: {} → {}", call.split_whitespace().collect::<Vec<_>>().join(" "), regen_call));
                    }
                    // and it reads back to itself
                    assert_eq!(luaparse::parse_entry(&regen).as_ref(), Some(&d), "{name}:{line}");
                    assert_eq!(luaparse::trigger_of(&src).as_deref(), Some(d.trigger.as_str()), "{name}:{line}");
                }
                None => nones.push(format!("{name}:{line}")),
            }
        }
    }
    eprintln!(
        "luaparse on her files: {total} entries in {} files; {drafts} parse to a Draft; {identical} regenerate identically (modulo whitespace/alignment), {same_meaning} as the same call with a different string spelling; {} are not drafts (None → raw Lua)",
        files.len(),
        total - drafts
    );
    eprintln!("spelled differently: {spelled:#?}");
    eprintln!("None: {}", nones.join(", "));
    assert!(total > 200);
    assert!(drafts * 10 >= total * 8, "{drafts}/{total}");
}

/// Entries the parser must not turn into a draft (they would regenerate
/// wrong), and some it must read right.
#[test]
fn parse_entry_none_or_right() {
    for src in [
        r#"post("dot", [[\dot{%1}]], "default", { priority = -1, check = not_c }),"#,
        r#"env("pmat", "pmatrix", "default"),"#,
        r#"text("dm", block, "changed", { line = "start" }),"#,
        r#"math("x", [[y]] .. "z", "o"),"#,
        r#"math("x", [[y]], "o", { tab = true }),"#,
        r#"text("x", [[y]], "o", { where = "display" }),"#,
        r#"pat("(%a)(%d)", [[%1_{%2}]], "yours", { name = "‹letter›‹digit›", eg = "x2" }),"#,
        r#"math("x", [[y]], "o", { boundary = "none" }),"#,
        r#"math("x", [[y]], "o", { priority = 1.5 }),"#,
        r#"math(x, [[y]], "o"),"#,
        r#"math("x", [[y]], "o", { check = function(a) return true end }),"#,
        r#"math("a", [[b]], "o"), math("c", [[d]], "o"),"#,
        r#"math("x", function() return "y" end, "o"),"#,
        r#"vis("x", [[%V]], "o", { tab = true }),"#,
        r#"text("x", [[y]], "o", { line = "middle" }),"#,
        r#"math("x", [[y]], "o", { ft = "md" }),"#,
        r#"math("x", [[y]], "o", { priority = 1, priority = 2 }),"#,
        r#"math("x", [[y]], "o", "extra", 4),"#,
        r#"local x = 1"#,
        r#"math("x", [[y]], "o"#,
    ] {
        assert_eq!(luaparse::parse_entry(src), None, "{src}");
    }
    let d = luaparse::parse_entry(r#"h.math("x", [==[a]]b]==], { desc = "d" }),"#).unwrap();
    assert_eq!((d.trigger.as_str(), d.origin.as_str(), d.desc.as_str()), ("x", "new", "d"));
    assert_eq!(d.output, vec![t("a]]b")]);
    let d = luaparse::parse_entry("text('q', \"\\\\x\\n\", 'castel', { tab = true, line = 'list item', ft = 'markdown', priority = -2 })").unwrap();
    assert_eq!(d.output, vec![t("\\x\n")]);
    assert_eq!((d.context, d.fires, d.line, d.ft, d.priority), (Context::Text, Fires::Tab, LineRule::ListItem, FileType::Markdown, -2));
}

// ---------------------------------------------------------------------------
// inbox.lua on disk (temp dirs only)

fn temp_paths(tag: &str) -> Paths {
    let base = std::env::temp_dir().join(format!("ntex-core-codegen-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(base.join("snippets/tex")).unwrap();
    Paths { config: base.clone(), snippets: base.join("snippets"), templates_tex: base.join("templates/tex"), state: base.join("state") }
}

fn ids(p: &Paths) -> Vec<String> {
    inbox::read(p).unwrap().into_iter().map(|e| e.id).collect()
}

#[test]
fn inbox_save_replace_remove() {
    let p = temp_paths("srr");
    let ff = draft("ff", parse_body(r"\frac{<>}{<>}"));
    let a = inbox::save(&p, &ff, "2026-10-07T14:32").unwrap();
    let b = inbox::save(&p, &ff, "2026-10-07T14:32").unwrap();
    let c = inbox::save(&p, &draft("//", parse_body(r"\frac{<>}{<>}")), "2026-10-07T14:32").unwrap();
    let thm = with("thm", parse_body("\\begin{theorem}\n\t<>\n\\end{theorem}"), |d| {
        d.context = Context::Text;
        d.fires = Fires::Tab;
        d.line = LineRule::Start;
        d.ft = FileType::Tex;
    });
    let e = inbox::save(&p, &thm, "2026-10-07T14:40").unwrap();
    assert_eq!(ids(&p), [a.clone(), b.clone(), c.clone(), e.clone()]);
    // ids are unique and readable
    assert_eq!(a, "ff-1432");
    assert_ne!(a, b);
    assert_eq!(c, "s-1432");
    // each entry reads back as its draft
    let list = inbox::read(&p).unwrap();
    let mut back = list[3].draft.clone().unwrap();
    assert_eq!(back.id.as_deref(), Some(e.as_str()));
    back.id = None;
    back.pinned.clear();
    assert_eq!(back, thm);
    assert_eq!(list[3].made, "2026-10-07T14:40");
    assert_eq!(list[3].trigger, "thm");
    // replace keeps id, made and place
    let mut edit = list[1].draft.clone().unwrap();
    edit.output = parse_body(r"\dfrac{<>}{<>}");
    let id = inbox::save(&p, &edit, "2026-10-07T15:00").unwrap();
    assert_eq!(id, b);
    let list = inbox::read(&p).unwrap();
    assert_eq!(list.iter().map(|e| e.id.clone()).collect::<Vec<_>>(), [a.clone(), b.clone(), c.clone(), e.clone()]);
    assert!(list[1].source.contains("dfrac"));
    assert_eq!(list[1].made, "2026-10-07T14:32");
    // remove
    inbox::remove(&p, &[a.clone(), e.clone()]).unwrap();
    assert_eq!(ids(&p), [b.clone(), c.clone()]);
    let text = std::fs::read_to_string(p.inbox()).unwrap();
    assert!(!text.contains("theorem"));
    assert!(text.contains("local math, math_tab, text, suffix, post, pat, vis = h.math"));
    assert!(text.trim_end().ends_with("})"));
    // removing what isn't there changes nothing
    inbox::remove(&p, &["nope".into()]).unwrap();
    assert_eq!(std::fs::read_to_string(p.inbox()).unwrap(), text);
    // only inbox.lua in the folder: no temp files left behind
    let names: Vec<String> = std::fs::read_dir(p.snippets.join("tex")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into()).collect();
    assert_eq!(names, ["inbox.lua"]);
    let _ = std::fs::remove_dir_all(&p.config);
}

#[test]
fn inbox_writes_are_atomic() {
    use std::os::unix::fs::MetadataExt;
    let p = temp_paths("atomic");
    inbox::save(&p, &draft("aa", parse_body(r"\alpha")), "2026-10-07T10:00").unwrap();
    let ino = std::fs::metadata(p.inbox()).unwrap().ino();
    // a reader polling the file never sees half of it
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let reader = {
        let (path, stop) = (p.inbox(), stop.clone());
        std::thread::spawn(move || {
            let mut reads = 0;
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                let text = std::fs::read_to_string(&path).unwrap();
                assert!(text.trim_end().ends_with("})"), "a partial file was visible:\n{text}");
                reads += 1;
            }
            reads
        })
    };
    for k in 0..150 {
        let d = draft(&format!("t{k}"), vec![t(&format!(r"\mathrm{{{}}}", "x".repeat(k * 7)))]);
        inbox::save(&p, &d, "2026-10-07T10:01").unwrap();
    }
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    assert!(reader.join().unwrap() > 0);
    // replaced by rename: a new file, not rewritten in place
    assert_ne!(std::fs::metadata(p.inbox()).unwrap().ino(), ino);
    let all = ids(&p);
    let mut uniq = all.clone();
    uniq.sort();
    uniq.dedup();
    assert_eq!(all.len(), 151);
    assert_eq!(uniq.len(), 151, "ids are unique");
    let _ = std::fs::remove_dir_all(&p.config);
}

#[test]
fn inbox_hand_edits() {
    let p = temp_paths("hand");
    let mut text = inbox::empty_file();
    text = text.replace(
        "{\n})",
        "{\n  -- my favourite\n  math(\"hh\", [[\\hbar]], \"yours\"), -- planck\n  env(\"pmat\", \"pmatrix\", \"yours\"),\n  -- studio: id=gone-0900 made=2026-10-07T09:00\n  -- studio: id=kk-0901 made=2026-10-07T09:01\n  math(\"kk\", [[\\kappa]], \"yours\")\n})",
    );
    std::fs::write(p.inbox(), &text).unwrap();
    let list = inbox::read(&p).unwrap();
    // hand-written entries without markers are listed, with stable ids
    assert_eq!(list.len(), 3);
    assert!(list[0].id.starts_with("hh-h"), "{}", list[0].id);
    assert_eq!(list[0].made, "");
    assert_eq!(list[0].trigger, "hh");
    assert!(list[0].draft.is_some());
    assert_eq!(list[1].trigger, "pmat");
    assert!(list[1].draft.is_none(), "env() opens as raw Lua");
    assert_eq!(inbox::read(&p).unwrap()[0].id, list[0].id);
    // the marker nearest the call wins; a stale one above it is not the id
    assert_eq!(list[2].id, "kk-0901");
    assert!(!list[2].source.contains("-- studio:"));
    // editing the unmarked one keeps her comments and gives it a marker
    let mut d = list[0].draft.clone().unwrap();
    d.output = parse_body(r"\hslash");
    let id = inbox::save(&p, &d, "2026-10-07T09:05").unwrap();
    assert_eq!(id, list[0].id);
    let after = std::fs::read_to_string(p.inbox()).unwrap();
    assert!(after.contains("-- my favourite"), "{after}");
    assert!(after.contains("[[\\hslash]], \"yours\"), -- planck"), "{after}");
    let list2 = inbox::read(&p).unwrap();
    assert_eq!(list2.len(), 3);
    assert_eq!(list2[0].id, id);
    assert!(list2[0].source.contains("hslash"));
    // the other unmarked entry got a marker too, so its id is fixed from now on
    assert_eq!(list2[1].id, "pmat-0905");
    // a new entry after a comma-less last entry
    let n = inbox::save(&p, &draft("zz", vec![t("z")]), "2026-10-07T09:06").unwrap();
    let list3 = inbox::read(&p).unwrap();
    assert_eq!(list3.len(), 4);
    assert_eq!(list3[3].id, n);
    assert!(std::fs::read_to_string(p.inbox()).unwrap().contains("[[\\kappa]], \"yours\"),"));
    // raw Lua with two calls stays one entry
    let raw = Draft { kind: Kind::RawLua, raw_lua: "math(\"r1\", [[a]], \"yours\"), math(\"r2\", [[b]], \"yours\")".into(), ..Draft::default() };
    let rid = inbox::save(&p, &raw, "2026-10-07T09:07").unwrap();
    let list4 = inbox::read(&p).unwrap();
    assert_eq!(list4.len(), 5);
    assert_eq!(list4[4].id, rid);
    assert!(list4[4].draft.is_none());
    // an unreadable file is kept, commented out, under a fresh inbox
    std::fs::write(p.inbox(), "this is not lua {").unwrap();
    inbox::save(&p, &draft("ff", vec![t("x")]), "2026-10-07T09:08").unwrap();
    let kept = std::fs::read_to_string(p.inbox()).unwrap();
    assert!(kept.contains("-- | this is not lua {"));
    assert_eq!(inbox::read(&p).unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(&p.config);
}
