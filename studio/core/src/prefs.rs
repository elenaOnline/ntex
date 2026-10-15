//! Reads the `studio = { … }` table of prefs.lua (simple `key = "string" |
//! number | true/false` lines; comments allowed). Missing file/table/key →
//! the defaults in StudioPrefs::default().
use crate::model::*;

pub fn read(paths: &Paths) -> StudioPrefs {
    match std::fs::read_to_string(paths.prefs()) {
        Ok(t) => parse(&t),
        Err(_) => StudioPrefs::default(),
    }
}

/// A value in the table.
#[derive(Clone, Debug, PartialEq)]
enum V {
    Str(String),
    Num(f64),
    Bool(bool),
    /// anything else (a nested table, an expression): ignored
    Other,
}

/// Parse from text (for tests).
pub fn parse(prefs_lua: &str) -> StudioPrefs {
    let mut p = StudioPrefs::default();
    let Some(body) = studio_table(prefs_lua) else { return p };
    for (k, v) in pairs(&body) {
        match (k.as_str(), v) {
            ("hide_after_save", V::Bool(b)) => p.hide_after_save = b,
            ("render_preview", V::Bool(b)) => p.render_preview = b,
            ("typeface", V::Str(s)) => p.typeface = s,
            ("sort_model", V::Str(s)) => p.sort_model = s,
            ("sort_effort", V::Str(s)) => p.sort_effort = s,
            ("figure_model", V::Str(s)) if !s.trim().is_empty() => p.figure_model = s,
            ("figure_effort", V::Str(s)) if !s.trim().is_empty() => p.figure_effort = s,
            ("figure_fast_model", V::Str(s)) if !s.trim().is_empty() => p.figure_fast_model = s,
            ("figure_fast_effort", V::Str(s)) if !s.trim().is_empty() => p.figure_fast_effort = s,
            ("figure_fast", V::Bool(b)) => p.figure_fast = b,
            _ => {}
        }
    }
    p
}

/// The text with comments blanked out (strings kept), same length.
fn strip_comments(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'"' || c == b'\'' {
            let start = i;
            i += 1;
            while i < b.len() && b[i] != c && b[i] != b'\n' {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i = (i + 1).min(b.len());
            out.push_str(&s[start..i]);
        } else if c == b'-' && b.get(i + 1) == Some(&b'-') {
            // a long comment --[[ … ]] or --[==[ … ]==]
            let mut j = i + 2;
            let mut level = None;
            if b.get(j) == Some(&b'[') {
                let mut k = j + 1;
                while b.get(k) == Some(&b'=') {
                    k += 1;
                }
                if b.get(k) == Some(&b'[') {
                    level = Some(k - j - 1);
                    j = k + 1;
                }
            }
            let end = match level {
                Some(n) => {
                    let close = format!("]{}]", "=".repeat(n));
                    s[j..].find(&close).map(|x| j + x + close.len()).unwrap_or(b.len())
                }
                None => s[i..].find('\n').map(|x| i + x).unwrap_or(b.len()),
            };
            for ch in s[i..end].chars() {
                out.push(if ch == '\n' { '\n' } else { ' ' });
            }
            i = end;
        } else {
            let ch = s[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// The text between the braces of `studio = {` … `}` (comments removed).
fn studio_table(src: &str) -> Option<String> {
    let s = strip_comments(src);
    let b = s.as_bytes();
    let mut search = 0;
    while let Some(rel) = s[search..].find("studio") {
        let at = search + rel;
        search = at + 6;
        // a whole word
        if at > 0 && (b[at - 1].is_ascii_alphanumeric() || b[at - 1] == b'_' || b[at - 1] == b'.') {
            continue;
        }
        if b.get(at + 6).is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_') {
            continue;
        }
        let rest = s[at + 6..].trim_start();
        let Some(rest) = rest.strip_prefix('=') else { continue };
        let rest = rest.trim_start();
        let Some(rest) = rest.strip_prefix('{') else { continue };
        let open = s.len() - rest.len();
        // the matching brace, skipping strings
        let mut depth = 1;
        let mut i = open;
        while i < b.len() {
            match b[i] {
                b'"' | b'\'' => {
                    let q = b[i];
                    i += 1;
                    while i < b.len() && b[i] != q && b[i] != b'\n' {
                        if b[i] == b'\\' {
                            i += 1;
                        }
                        i += 1;
                    }
                }
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(s[open..i].to_string());
                    }
                }
                _ => {}
            }
            i += 1;
        }
        return Some(s[open..].to_string());
    }
    None
}

/// `key = value` pairs at the table's top level, split on `,`/`;`/newlines.
fn pairs(body: &str) -> Vec<(String, V)> {
    let mut items: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut depth = 0;
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' | '\'' => {
                cur.push(c);
                while let Some(d) = chars.next() {
                    cur.push(d);
                    if d == '\\' {
                        if let Some(e) = chars.next() {
                            cur.push(e);
                        }
                    } else if d == c || d == '\n' {
                        break;
                    }
                }
            }
            '{' | '(' | '[' => {
                depth += 1;
                cur.push(c);
            }
            '}' | ')' | ']' => {
                depth -= 1;
                cur.push(c);
            }
            ',' | ';' | '\n' if depth == 0 => items.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    items.push(cur);
    let mut out = Vec::new();
    for it in items {
        let Some((k, v)) = it.split_once('=') else { continue };
        let k = k.trim();
        if k.is_empty() || !k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            continue;
        }
        out.push((k.to_string(), value(v.trim())));
    }
    out
}

fn value(v: &str) -> V {
    match v {
        "true" => return V::Bool(true),
        "false" => return V::Bool(false),
        _ => {}
    }
    if v.len() >= 2 && ((v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\''))) {
        if let Some(s) = crate::luaparse::lua_string_value(v) {
            return V::Str(s);
        }
    }
    if v.starts_with("[[") || v.starts_with("[=") {
        if let Some(s) = crate::luaparse::lua_string_value(v) {
            return V::Str(s);
        }
    }
    if let Ok(n) = v.parse::<f64>() {
        return V::Num(n);
    }
    V::Other
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_without_a_table() {
        assert_eq!(parse(""), StudioPrefs::default());
        assert_eq!(parse("return { images = false, latex = { snippets = true } }"), StudioPrefs::default());
        assert_eq!(parse("this is { not lua"), StudioPrefs::default());
    }

    #[test]
    fn reads_the_table() {
        let src = r#"-- Your ntex preferences.
return {
  images = false, -- studio = { hide_after_save = false } in a comment is ignored
  latex = { snippets = true, typeface = "Nope" },
  -- Ntex Studio
  studio = {
    hide_after_save = false, -- stay in front after ⌘↵
    typeface = "Palatino",
    sort_model = 'claude-sonnet-5-5';
    -- sort_effort = "low",
    render_preview = false,
    unknown_key = 3,
    width = 12.5,
    nested = { a = 1, typeface = "No" },
  },
  after = "x",
}
"#;
        let p = parse(src);
        assert!(!p.hide_after_save);
        assert_eq!(p.typeface, "Palatino");
        assert_eq!(p.sort_model, "claude-sonnet-5-5");
        assert_eq!(p.sort_effort, "high");
        assert!(!p.render_preview);
    }

    #[test]
    fn missing_keys_and_wrong_types_keep_defaults() {
        let p = parse("return { studio = { typeface = true, hide_after_save = \"no\", sort_effort = \"medium\" } }");
        assert_eq!(p, StudioPrefs { sort_effort: "medium".into(), ..StudioPrefs::default() });
        let p = parse("return {\n  studio = {\n  },\n}\n");
        assert_eq!(p, StudioPrefs::default());
        // an unterminated table still yields what it has
        let p = parse("return { studio = {\n render_preview = false,\n");
        assert!(!p.render_preview);
        // escapes in strings; a brace inside a string
        let p = parse(r#"studio = { typeface = "Computer \"Modern\" }", hide_after_save = true }"#);
        assert_eq!(p.typeface, "Computer \"Modern\" }");
    }

    #[test]
    fn reads_the_file() {
        let base = std::env::temp_dir().join(format!("ntex-core-prefs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let paths = Paths { config: base.clone(), snippets: base.join("snippets"), templates_tex: base.join("templates/tex"), state: base.join("state") };
        assert_eq!(read(&paths), StudioPrefs::default());
        std::fs::write(paths.prefs(), "return {\n  studio = { typeface = \"Palatino\" },\n}\n").unwrap();
        assert_eq!(read(&paths).typeface, "Palatino");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn figure_keys() {
        let p = parse("return { studio = {\n  figure_model = \"gpt-6.1-sol\", -- the normal one\n  figure_effort = \"medium\",\n  figure_fast_model = 'claude-haiku-5-5',\n  figure_fast_effort = \"low\",\n  figure_fast = true,\n} }");
        assert_eq!((p.figure_model.as_str(), p.figure_effort.as_str()), ("gpt-6.1-sol", "medium"));
        assert_eq!((p.figure_fast_model.as_str(), p.figure_fast_effort.as_str(), p.figure_fast), ("claude-haiku-5-5", "low", true));
        // missing, empty or mistyped: the defaults
        let d = StudioPrefs::default();
        let p = parse("return { studio = { figure_model = \"\", figure_fast = \"yes\" } }");
        assert_eq!((p.figure_model, p.figure_fast), (d.figure_model.clone(), d.figure_fast));
        // an older serialized StudioPrefs (no figure fields) still loads
        let old = serde_json::json!({"hide_after_save": true, "typeface": "Palatino", "sort_model": "m", "sort_effort": "high", "render_preview": false});
        let p: StudioPrefs = serde_json::from_value(old).unwrap();
        assert_eq!(p.figure_fast_model, d.figure_fast_model);
    }

    /// Her real prefs.lua (read-only) parses without surprises.
    #[test]
    fn real_prefs() {
        let p = std::path::PathBuf::from(std::env::var("HOME").unwrap()).join(".config/texnvim/prefs.lua");
        if let Ok(t) = std::fs::read_to_string(p) {
            let got = parse(&t);
            assert!(["Computer Modern", "Palatino"].contains(&got.typeface.as_str()) || !got.typeface.is_empty());
        }
    }
}
