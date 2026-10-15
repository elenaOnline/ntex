//! ADVERSARY shared helpers: expected.json, TeX-token comparison (my own
//! Unicode table, from FIGURES.md, not core::unicode), JSON matching,
//! diagnostics, failure reports.
#![allow(dead_code)]
use ntex_figures::model::*;
use serde_json::{Value, json};
use std::path::PathBuf;

pub fn crate_dir() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")) }

pub fn load_expected() -> Value {
    let p = crate_dir().join("verification/expected.json");
    serde_json::from_str(&std::fs::read_to_string(&p).expect("verification/expected.json")).unwrap()
}

pub fn write_out(name: &str, v: &Value) {
    let dir = crate_dir().join("verification/out");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(name), serde_json::to_string_pretty(v).unwrap()).unwrap();
}

pub fn catch<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    std::panic::set_hook(prev);
    r.map_err(|e| {
        e.downcast_ref::<String>().cloned()
            .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_else(|| "panic".into())
    })
}

pub struct Failure {
    pub id: String,
    pub tier: String,
    pub check: String,
    pub src: String,
    pub expected: Value,
    pub actual: Value,
}

pub fn print_failures(what: &str, fs: &[Failure]) {
    for f in fs {
        println!("\n[{what} {} {}] {}", f.id, f.tier, f.check);
        println!("  input:    {}", f.src.replace('\n', "⏎ ").replace('\r', "␍"));
        println!("  expected: {}", short(&f.expected));
        println!("  actual:   {}", short(&f.actual));
    }
}

fn short(v: &Value) -> String {
    let s = v.to_string();
    if s.chars().count() > 900 { format!("{}…", s.chars().take(900).collect::<String>()) } else { s }
}

pub fn failures_json(fs: &[Failure]) -> Value {
    json!(fs.iter().map(|f| json!({"id": f.id, "tier": f.tier, "check": f.check, "src": f.src,
        "expected": f.expected, "actual": f.actual})).collect::<Vec<_>>())
}

// ------------------------------------------------------------------ TeX

fn uni(c: char) -> Option<&'static str> {
    Some(match c {
        'α' => "\\alpha ", 'β' => "\\beta ", 'γ' => "\\gamma ", 'δ' => "\\delta ", 'ε' => "\\varepsilon ",
        'ζ' => "\\zeta ", 'η' => "\\eta ", 'θ' => "\\theta ", 'ι' => "\\iota ", 'κ' => "\\kappa ",
        'λ' => "\\lambda ", 'μ' => "\\mu ", 'ν' => "\\nu ", 'ξ' => "\\xi ", 'π' => "\\pi ", 'ρ' => "\\rho ",
        'σ' => "\\sigma ", 'τ' => "\\tau ", 'υ' => "\\upsilon ", 'φ' => "\\varphi ", 'χ' => "\\chi ",
        'ψ' => "\\psi ", 'ω' => "\\omega ", 'Γ' => "\\Gamma ", 'Δ' => "\\Delta ", 'Θ' => "\\Theta ",
        'Λ' => "\\Lambda ", 'Ξ' => "\\Xi ", 'Π' => "\\Pi ", 'Σ' => "\\Sigma ", 'Υ' => "\\Upsilon ",
        'Φ' => "\\Phi ", 'Ψ' => "\\Psi ", 'Ω' => "\\Omega ",
        'ℤ' => "\\mathbb{Z}", 'ℚ' => "\\mathbb{Q}", 'ℝ' => "\\mathbb{R}", 'ℂ' => "\\mathbb{C}", 'ℕ' => "\\mathbb{N}",
        '∘' => "\\circ ", '×' => "\\times ", '⊗' => "\\otimes ", '⊕' => "\\oplus ", '≅' => "\\cong ",
        '⊂' => "\\subset ", '⊆' => "\\subseteq ", '∈' => "\\in ", '∉' => "\\notin ", '∩' => "\\cap ",
        '∪' => "\\cup ", '∅' => "\\emptyset ", '∞' => "\\infty ", '≤' => "\\leq ", '≥' => "\\geq ",
        '≠' => "\\neq ", '↦' => "\\mapsto ", '→' => "\\to ",
        _ => return None,
    })
}

fn sub(c: char) -> Option<char> {
    let s = "₀₁₂₃₄₅₆₇₈₉";
    s.chars().position(|x| x == c).map(|i| (b'0' + i as u8) as char)
}
fn sup(c: char) -> Option<char> {
    let s = "⁰¹²³⁴⁵⁶⁷⁸⁹";
    s.chars().position(|x| x == c).map(|i| (b'0' + i as u8) as char)
}

/// My Unicode → LaTeX (FIGURES.md's table), used only to compare.
pub fn my_latex(s: &str) -> String {
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < cs.len() {
        if sub(cs[i]).is_some() {
            out.push_str("_{");
            while i < cs.len() && let Some(d) = sub(cs[i]) { out.push(d); i += 1; }
            out.push('}');
            continue;
        }
        if sup(cs[i]).is_some() {
            out.push_str("^{");
            while i < cs.len() && let Some(d) = sup(cs[i]) { out.push(d); i += 1; }
            out.push('}');
            continue;
        }
        match uni(cs[i]) { Some(t) => out.push_str(t), None => out.push(cs[i]) }
        i += 1;
    }
    out
}

pub fn tex_tokens(s: &str) -> Vec<String> {
    let s = my_latex(s);
    let cs: Vec<char> = s.chars().collect();
    let mut toks: Vec<String> = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c.is_whitespace() { i += 1; continue; }
        if c == '\\' && i + 1 < cs.len() {
            let mut j = i + 1;
            if cs[j].is_ascii_alphabetic() {
                while j < cs.len() && cs[j].is_ascii_alphabetic() { j += 1; }
            } else { j += 1; }
            toks.push(cs[i..j].iter().collect());
            i = j;
            continue;
        }
        toks.push(c.to_string());
        i += 1;
    }
    for t in toks.iter_mut() {
        let a = match t.as_str() {
            "\\le" => "\\leq", "\\ge" => "\\geq", "\\ne" => "\\neq", "\\rightarrow" => "\\to",
            "\\varnothing" => "\\emptyset", _ => continue,
        };
        *t = a.to_string();
    }
    // ^{x} → ^x for a single token
    let mut out: Vec<String> = Vec::new();
    let mut k = 0;
    while k < toks.len() {
        if (toks[k] == "^" || toks[k] == "_") && k + 3 < toks.len() && toks[k + 1] == "{" && toks[k + 3] == "}" {
            out.push(toks[k].clone());
            out.push(toks[k + 2].clone());
            k += 4;
            continue;
        }
        out.push(toks[k].clone());
        k += 1;
    }
    out
}

pub fn tex_eq(a: &str, b: &str) -> bool { tex_tokens(a) == tex_tokens(b) }
pub fn norm_key(a: &str) -> String { tex_tokens(a).join("\u{1}") }

/// expected ⊑ actual: objects are subset-matched, {"any_of": [...]} is a
/// choice, strings compare as TeX, numbers within 1e-3.
pub fn matches(w: &Value, a: &Value) -> bool {
    match w {
        Value::Object(m) if m.len() == 1 && m.contains_key("any_of") => {
            m["any_of"].as_array().unwrap().iter().any(|x| matches(x, a))
        }
        Value::Null => a.is_null(),
        Value::Bool(b) => a.as_bool() == Some(*b),
        Value::Number(n) => a.as_f64().is_some_and(|x| (x - n.as_f64().unwrap()).abs() <= 1e-3),
        Value::String(s) => a.as_str().is_some_and(|x| tex_eq(s, x)),
        Value::Array(v) => a.as_array().is_some_and(|x| x.len() == v.len() && v.iter().zip(x).all(|(p, q)| matches(p, q))),
        Value::Object(m) => a.is_object() && m.iter().all(|(k, v)| matches(v, a.get(k).unwrap_or(&Value::Null))),
    }
}

// ------------------------------------------------------------------ diagnostics

pub fn diag_brief(d: &[Diagnostic]) -> Value {
    json!(d.iter().map(|x| format!("{:?} @{}: {}", x.severity, x.span.line, x.message)).collect::<Vec<_>>())
}

pub fn check_diags(spec: &Value, diags: &[Diagnostic], fails: &mut Vec<(String, Value, Value)>) {
    let errs = diags.iter().filter(|d| d.severity == Severity::Error).count();
    let warns = diags.iter().filter(|d| d.severity == Severity::Warning).count();
    if spec.get("clean").is_some() && errs + warns > 0 {
        fails.push(("diagnostics: clean".into(), json!("no errors or warnings"), diag_brief(diags)));
    }
    if spec.get("no_errors").is_some() && errs > 0 {
        fails.push(("diagnostics: no errors".into(), json!("no Error"), diag_brief(diags)));
    }
    let on = |l: i64, sev: Option<Severity>| diags.iter().any(|d| d.span.line as i64 == l && sev.is_none_or(|s| d.severity == s));
    for l in spec["error_lines"].as_array().cloned().unwrap_or_default() {
        let l = l.as_i64().unwrap();
        if !on(l, Some(Severity::Error)) {
            fails.push((format!("diagnostics: Error on line {l}"), json!(l), diag_brief(diags)));
        }
    }
    for l in spec["warn_lines"].as_array().cloned().unwrap_or_default() {
        let l = l.as_i64().unwrap();
        if !on(l, Some(Severity::Warning)) {
            fails.push((format!("diagnostics: Warning on line {l}"), json!(l), diag_brief(diags)));
        }
    }
    for l in spec["any_lines"].as_array().cloned().unwrap_or_default() {
        let l = l.as_i64().unwrap();
        if !on(l, None) {
            fails.push((format!("diagnostics: something on line {l}"), json!(l), diag_brief(diags)));
        }
    }
    for d in diags {
        if !d.message.starts_with(&format!("line {}:", d.span.line)) {
            fails.push(("soft: message format `line N: …`".into(), json!(format!("line {}: …", d.span.line)), json!(d.message)));
        }
    }
}

/// Every string in a JSON value (to look for leaked control characters).
pub fn any_string(v: &Value, f: &dyn Fn(&str) -> bool) -> bool {
    match v {
        Value::String(s) => f(s),
        Value::Array(a) => a.iter().any(|x| any_string(x, f)),
        Value::Object(m) => m.iter().any(|(k, x)| k != "source" && k != "text" && any_string(x, f)),
        _ => false,
    }
}
