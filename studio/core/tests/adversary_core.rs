//! ADVERSARY: independent acceptance for ntex-studio-core.
//!
//! Data: ../verification/expected.json (derived from helpers.lua, README.md,
//! LuaSnip and ntex-notes.sty, with every "you get" measured in a real headless
//! ntex; see ../verification/acceptance.md). End-to-end checks load the Lua that
//! *this crate* generates into a real headless ntex (verification/try.lua,
//! the same harness as tests/driver.lua) and type the trigger.
//!
//! Headless only: nvim --headless (no UI), tectonic (no viewer), gs txtwrite
//! (text out of the PDF). Everything is written under a temp dir; her real files
//! are only ever read (copied).
//!
//! Each test collects every failure and panics once with the full list, so one
//! run reports everything: `cargo test -p ntex-studio-core --test adversary_core`.

use ntex_studio_core::model::*;
use ntex_studio_core::{apply, blocks, codegen, colour, inbox, infer, luaparse};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

// ---------------------------------------------------------------- harness

fn studio() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}
fn verification() -> PathBuf {
    studio().join("verification")
}
fn config_real() -> PathBuf {
    studio().parent().unwrap().to_path_buf()
}

fn expected() -> &'static Value {
    static E: OnceLock<Value> = OnceLock::new();
    E.get_or_init(|| {
        let s = fs::read_to_string(verification().join("expected.json")).expect("verification/expected.json");
        serde_json::from_str(&s).unwrap()
    })
}

fn tmpdir(tag: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!(
        "ntex-adversary-{}-{}-{}",
        tag,
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    fs::create_dir_all(&base).unwrap();
    base
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let p = e.path();
        let t = to.join(e.file_name());
        if p.is_dir() {
            copy_dir(&p, &t);
        } else {
            fs::copy(&p, &t).unwrap();
        }
    }
}

/// A temp copy of ~/.config/texnvim's snippets + templates, as `Paths`.
fn temp_paths(tag: &str) -> Paths {
    let root = tmpdir(tag);
    let config = root.join("texnvim");
    copy_dir(&config_real().join("snippets"), &config.join("snippets"));
    copy_dir(&config_real().join("templates/tex"), &config.join("templates/tex"));
    let _ = fs::remove_file(config.join("snippets/tex/inbox.lua")); // start from a known state
    let state = root.join("state");
    fs::create_dir_all(&state).unwrap();
    Paths { templates_tex: config.join("templates/tex"), snippets: config.join("snippets"), config, state }
}

/// Run verification/try.lua (a headless ntex) over `cases`; returns its results.
fn ntex(snippets: &Path, cases: &Value) -> Vec<Value> {
    let dir = tmpdir("try");
    let inp = dir.join("in.json");
    let out = dir.join("out.json");
    fs::write(&inp, serde_json::to_string(cases).unwrap()).unwrap();
    let st = Command::new("nvim")
        .args(["--clean", "--headless", "-l"])
        .arg(verification().join("try.lua"))
        .arg(&inp)
        .arg(&out)
        .env("TEXNVIM_SNIPPETS", snippets)
        .env("TEXNVIM_NO_VIEWER", "1")
        .env("TEXNVIM_NO_COMPILE", "1")
        .env("TEXNVIM_NO_REFERENCE", "1")
        .env("TEXNVIM_NO_LSP", "1")
        .output()
        .expect("nvim");
    let s = fs::read_to_string(&out).unwrap_or_else(|_| {
        panic!("try.lua produced nothing: {}\n{}", String::from_utf8_lossy(&st.stdout), String::from_utf8_lossy(&st.stderr))
    });
    let _ = fs::remove_dir_all(&dir);
    serde_json::from_str::<Vec<Value>>(&s).unwrap()
}

fn report(test: &str, fails: &[String], total: usize) {
    let dir = verification().join("results");
    let _ = fs::create_dir_all(&dir);
    let mut s = format!("{test}: {} of {} checks failed\n\n", fails.len(), total);
    for f in fails {
        s.push_str(f);
        s.push_str("\n\n");
    }
    let _ = fs::write(dir.join(format!("{test}.txt")), &s);
    if !fails.is_empty() {
        panic!("{s}");
    }
}

fn draft_of(v: &Value) -> Draft {
    serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("draft json: {e}: {v}"))
}
fn strip(d: &Draft) -> Draft {
    let mut d = d.clone();
    d.id = None;
    d.pinned.clear();
    d
}
fn known() -> Vec<KnownSnippet> {
    serde_json::from_value(expected()["known"].clone()).unwrap()
}
fn q(s: &str) -> String {
    format!("{s:?}")
}

/// Compare measured steps with expected steps; push failures.
fn cmp_steps(label: &str, entry: &str, want: &Value, got: &Value, fails: &mut Vec<String>) -> usize {
    let mut n = 0;
    if let Some(load) = got.get("load") {
        if load["ok"] != json!(true) {
            fails.push(format!("{label}: the generated Lua does not load in ntex\n  entry: {entry}\n  error: {}", load["err"]));
            return 1;
        }
    }
    let ws = want.as_array().unwrap();
    let gs = got["steps"].as_array().cloned().unwrap_or_default();
    for (k, w) in ws.iter().enumerate() {
        n += 1;
        let Some(g) = gs.get(k) else {
            fails.push(format!("{label} step {k}: missing"));
            continue;
        };
        let mut bad = vec![];
        if w["buffer"] != g["buffer"] {
            bad.push(format!("buffer {} (expected {})", g["buffer"], w["buffer"]));
        }
        if w["mode"] != g["mode"] {
            bad.push(format!("mode {} (expected {})", g["mode"], w["mode"]));
        }
        if !w["sel"].is_null() && w["sel"] != g["sel"] {
            bad.push(format!("sel {} (expected {})", g["sel"], w["sel"]));
        }
        let wexp: Vec<&str> = w["expanded"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
        let gexp: Vec<&str> = g["expanded"].as_array().map(|a| a.iter().filter_map(|x| x.get("file").and_then(|f| f.as_str())).collect()).unwrap_or_default();
        if wexp != gexp {
            bad.push(format!("expanded from {gexp:?} (expected {wexp:?})"));
        }
        if !bad.is_empty() {
            fails.push(format!("{label} typing {}: {}\n  entry: {entry}", w["keys"], bad.join("; ")));
        }
    }
    n
}

// ---------------------------------------------------------------- A: codegen

#[test]
fn a_codegen_entries() {
    let mut fails = vec![];
    let mut n = 0;
    for c in expected()["snippets"].as_array().unwrap() {
        let d = draft_of(&c["draft"]);
        let got = codegen::entry(&d);
        if c["semantic"] == json!(true) {
            continue; // checked end to end only
        }
        n += 1;
        let mut ok = vec![c["entry"].as_str().unwrap().to_string()];
        ok.extend(c["alts"].as_array().unwrap().iter().map(|a| a.as_str().unwrap().to_string()));
        if !ok.contains(&got) {
            fails.push(format!(
                "{} ({}): codegen::entry\n  expected: {}\n  actual:   {}",
                c["id"].as_str().unwrap(),
                c["title"].as_str().unwrap(),
                q(&ok[0]),
                q(&got)
            ));
        }
    }
    report("a_codegen_entries", &fails, n);
}

#[test]
fn a_luaparse_roundtrip() {
    let mut fails = vec![];
    let mut n = 0;
    for c in expected()["snippets"].as_array().unwrap() {
        let id = c["id"].as_str().unwrap();
        let d = draft_of(&c["draft"]);
        if d.kind == Kind::RawLua {
            continue;
        }
        n += 2;
        let src = codegen::entry(&d);
        match luaparse::parse_entry(&src) {
            Some(p) if strip(&p) == strip(&d) => {}
            None if !d.check.is_empty() => {} // check functions may reopen as RawLua (luaparse contract)
            other => fails.push(format!("{id}: parse_entry(entry(draft)) != draft\n  entry: {}\n  got: {:?}", q(&src), other.map(|x| strip(&x)))),
        }
        // also from MY expected entry (the text she might have by hand)
        let mine = c["entry"].as_str().unwrap();
        if c["semantic"] != json!(true) {
            match luaparse::parse_entry(mine) {
                Some(p) if strip(&p) == strip(&d) => {}
                None if !d.check.is_empty() => {}
                other => fails.push(format!("{id}: parse_entry(expected entry) != draft\n  entry: {}\n  got: {:?}", q(mine), other.map(|x| strip(&x)))),
            }
        }
        n += 1;
        if luaparse::trigger_of(&src).as_deref() != Some(d.trigger.as_str()) {
            fails.push(format!("{id}: trigger_of({}) = {:?}, expected {:?}", q(&src), luaparse::trigger_of(&src), d.trigger));
        }
    }
    report("a_luaparse_roundtrip", &fails, n);
}

/// Every A-case: generate with codegen, load beside all her snippets in a real
/// headless ntex, type it, compare with the measured/derived expansion.
#[test]
fn a_end_to_end_in_real_ntex() {
    let paths = temp_paths("e2e");
    let mut cases = vec![];
    let mut meta = vec![];
    for c in expected()["snippets"].as_array().unwrap() {
        let d = draft_of(&c["draft"]);
        let entry = codegen::entry(&d);
        for (k, t) in c["tries"].as_array().unwrap().iter().enumerate() {
            let keys: Vec<Value> = t["steps"].as_array().unwrap().iter().map(|s| s["keys"].clone()).collect();
            let mut item = json!({"name": format!("{}#{k}", c["id"].as_str().unwrap()), "entry": entry, "ft": t["ft"], "text": t["text"], "steps": keys});
            if !t["select"].is_null() {
                item["select"] = t["select"].clone();
            }
            cases.push(item);
            meta.push((format!("{}#{k} ({})", c["id"].as_str().unwrap(), c["title"].as_str().unwrap()), entry.clone(), t["steps"].clone()));
        }
    }
    // F-2 style: after everything (including a trailing-newline case), a plain case must still work
    let ff = codegen::entry(&draft_of(&expected()["snippets"][0]["draft"]));
    cases.push(json!({"name": "after-all", "entry": ff, "ft": "tex", "text": "$‸$", "steps": ["ff"]}));
    meta.push(("A1 again after every other case (session not poisoned)".into(), ff, json!([{"keys": "ff", "buffer": "$\\frac{‸}{}$", "mode": "i", "sel": null, "expanded": ["zz_adv.lua"]}])));
    let got = ntex(&paths.snippets, &Value::Array(cases));
    let mut fails = vec![];
    let mut n = 0;
    for ((label, entry, want), g) in meta.iter().zip(got.iter()) {
        n += cmp_steps(label, entry, want, g, &mut fails);
    }
    report("a_end_to_end_in_real_ntex", &fails, n);
}

// ---------------------------------------------------------------- B

#[test]
fn b_parse_body_renumber_lua_string() {
    let mut fails = vec![];
    let mut n = 0;
    for c in expected()["parse_body"].as_array().unwrap() {
        n += 1;
        let want: Vec<Piece> = serde_json::from_value(c["pieces"].clone()).unwrap();
        let got = codegen::parse_body(c["input"].as_str().unwrap());
        if got != want {
            fails.push(format!("parse_body({})\n  expected: {want:?}\n  actual:   {got:?}", q(c["input"].as_str().unwrap())));
        }
        // and body() of those pieces must parse back to the same pieces (helper syntax is lossless)
        n += 1;
        let back = codegen::parse_body(&codegen::body(&want));
        let mut w2 = want.clone();
        codegen::renumber(&mut w2);
        let mut b2 = back.clone();
        codegen::renumber(&mut b2);
        if b2 != w2 {
            fails.push(format!("parse_body(body(p)) != p for {want:?}\n  body: {}\n  back: {back:?}", q(&codegen::body(&want))));
        }
    }
    for c in expected()["renumber"].as_array().unwrap() {
        n += 1;
        let mut p: Vec<Piece> = serde_json::from_value(c["input"].clone()).unwrap();
        let want: Vec<Piece> = serde_json::from_value(c["output"].clone()).unwrap();
        let before = p.clone();
        codegen::renumber(&mut p);
        if p != want {
            fails.push(format!("renumber({before:?})\n  expected: {want:?}\n  actual:   {p:?}"));
        }
    }
    // lua_string evaluates back to the string in LuaJIT
    let strs: Vec<String> = expected()["lua_strings"].as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect();
    let lits: Vec<String> = strs.iter().map(|s| codegen::lua_string(s)).collect();
    let dir = tmpdir("luastr");
    fs::write(dir.join("in.json"), serde_json::to_string(&lits).unwrap()).unwrap();
    let st = Command::new("nvim")
        .args(["--clean", "--headless", "-l"])
        .arg(verification().join("eval_lua.lua"))
        .arg(dir.join("in.json"))
        .arg(dir.join("out.json"))
        .output()
        .unwrap();
    let res: Vec<Value> = serde_json::from_str(&fs::read_to_string(dir.join("out.json")).unwrap_or_else(|_| panic!("{st:?}"))).unwrap();
    for ((s, lit), r) in strs.iter().zip(&lits).zip(&res) {
        n += 1;
        if r["ok"] != json!(true) || r["value"].as_str() != Some(s.as_str()) {
            fails.push(format!("lua_string({}) = {} evaluates to {} (error {})", q(s), lit, r["value"], r["err"]));
        }
    }
    let _ = fs::remove_dir_all(&dir);
    report("b_parse_body_renumber_lua_string", &fails, n);
}

#[test]
fn b_guess_check_try_plan() {
    let mut fails = vec![];
    let mut n = 0;
    for c in expected()["guess"].as_array().unwrap() {
        n += 1;
        let mut d = draft_of(&c["draft"]);
        let place: Option<Context> = serde_json::from_value(c["place"].clone()).unwrap();
        infer::guess(&mut d, place);
        let dv = serde_json::to_value(&d).unwrap();
        for (k, v) in c["expect"].as_object().unwrap() {
            if &dv[k] != v {
                fails.push(format!("guess: {}: {k} = {} (expected {v})", c["why"].as_str().unwrap(), dv[k]));
            }
        }
    }
    let k = known();
    for c in expected()["check"].as_array().unwrap() {
        n += 1;
        let d = draft_of(&c["draft"]);
        let f = infer::check(&d, &k);
        let codes: Vec<&str> = f.iter().map(|x| x.code.as_str()).collect();
        let has_err = f.iter().any(|x| x.severity == Severity::Error);
        for want in c["codes"].as_array().unwrap() {
            if !codes.contains(&want.as_str().unwrap()) {
                fails.push(format!("check: {}: no {want} finding; got {:?}", c["why"].as_str().unwrap(), f));
            }
        }
        if let Some(e) = c["errors"].as_bool() {
            if e != has_err {
                // the default-holding-'>' case may instead be handled by codegen: verified end to end
                if c["draft"]["trigger"] == json!("gtd") && !has_err {
                    let paths = temp_paths("gtd");
                    let entry = codegen::entry(&d);
                    let g = ntex(&paths.snippets, &json!([{"name": "gtd", "entry": entry, "ft": "tex", "text": "$‸$", "steps": ["gtd"]}]));
                    let s = &g[0]["steps"][0];
                    if s["sel"] != json!("a>b") {
                        fails.push(format!("check: a tab stop default 'a>b' gets no Error, and ntex shows {} sel={} (entry {})", s["buffer"], s["sel"], q(&entry)));
                    }
                    continue;
                }
                fails.push(format!("check: {}: has Error = {has_err}, expected {e}; findings {:?}", c["why"].as_str().unwrap(), f));
            }
        }
    }
    for c in expected()["try_plan"].as_array().unwrap() {
        n += 1;
        let d = draft_of(&c["draft"]);
        let t = infer::try_plan(&d);
        let e = &c["expect"];
        let mut bad = vec![];
        if let Some(v) = e.get("ft") { if t.ft != v.as_str().unwrap() { bad.push(format!("ft {:?} (expected {v})", t.ft)); } }
        if let Some(v) = e.get("place") { if t.place != v.as_str().unwrap() { bad.push(format!("place {:?} (expected {v})", t.place)); } }
        if let Some(v) = e.get("keys") { if t.keys != v.as_str().unwrap() { bad.push(format!("keys {:?} (expected {v})", t.keys)); } }
        if let Some(v) = e.get("tab") { if t.tab != v.as_bool().unwrap() { bad.push(format!("tab {} (expected {v})", t.tab)); } }
        if let Some(v) = e.get("nfills") { if t.fills.len() as u64 != v.as_u64().unwrap() { bad.push(format!("{} fills (expected {v})", t.fills.len())); } }
        if let Some(v) = e.get("keys_endswith") { if !t.keys.ends_with(v.as_str().unwrap()) { bad.push(format!("keys {:?} (expected …{v})", t.keys)); } }
        if let Some(v) = e.get("keys_len") { if t.keys.chars().count() as u64 != v.as_u64().unwrap() { bad.push(format!("keys {:?} (expected {v} chars)", t.keys)); } }
        if !bad.is_empty() {
            fails.push(format!("try_plan(trigger {:?}, {:?}/{:?}/{:?}): {}", d.trigger, d.kind, d.context, d.fires, bad.join("; ")));
        }
    }
    report("b_guess_check_try_plan", &fails, n);
}

// ---------------------------------------------------------------- C: conflicts

#[test]
fn c_conflicts_with_her_snippets() {
    let k = known();
    let mut fails = vec![];
    let mut n = 0;
    let mut cases = vec![];
    let mut meta = vec![];
    for c in expected()["conflicts"].as_array().unwrap() {
        n += 1;
        let id = c["id"].as_str().unwrap();
        let d = draft_of(&c["draft"]);
        let f = infer::check(&d, &k);
        let codes: Vec<&str> = f.iter().map(|x| x.code.as_str()).collect();
        let want: Vec<&str> = c["findings"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
        for w in &want {
            if !codes.contains(w) {
                fails.push(format!("{id} ({}): infer::check has no {w:?}; codes {codes:?}", c["title"].as_str().unwrap()));
            }
        }
        if want.is_empty() {
            for bad in ["shadows", "shadowed-by", "duplicate"] {
                if codes.contains(&bad) {
                    fails.push(format!("{id} ({}): false {bad:?} finding: {:?}", c["title"].as_str().unwrap(), f));
                }
            }
        }
        if want.contains(&"duplicate") {
            n += 1;
            let fix_ok = f.iter().filter(|x| x.code == "duplicate").any(|x| x.fix.as_ref().and_then(|fx| fx.draft.as_ref()).map(|d| d.priority >= 1).unwrap_or(false));
            if !fix_ok {
                fails.push(format!("{id}: the duplicate finding has no fix raising priority (measured: priority 1 wins, 0 loses after a reload): {:?}", f));
            }
        }
        let entry = codegen::entry(&d);
        for (j, t) in c["tries"].as_array().unwrap().iter().enumerate() {
            let keys: Vec<Value> = t["with"].as_array().unwrap().iter().map(|s| s["keys"].clone()).collect();
            let mut item = json!({"name": format!("{id}#{j}"), "entry": entry, "ft": t["ft"], "text": t["text"], "steps": keys});
            if !t["select"].is_null() {
                item["select"] = t["select"].clone();
            }
            cases.push(item);
            meta.push((format!("{id}#{j} ({})", c["title"].as_str().unwrap()), entry.clone(), t["with"].clone()));
        }
    }
    let paths = temp_paths("conf");
    let got = ntex(&paths.snippets, &Value::Array(cases));
    for ((label, entry, want), g) in meta.iter().zip(got.iter()) {
        n += cmp_steps(label, entry, want, g, &mut fails);
    }
    report("c_conflicts_with_her_snippets", &fails, n);
}

// ---------------------------------------------------------------- inbox

#[test]
fn inbox_file_roundtrip_and_loads() {
    let mut fails = vec![];
    let mut n = 0;
    let paths = temp_paths("inbox");
    let empty = inbox::empty_file();
    n += 4;
    if !empty.contains("Snippets made in Ntex Studio, waiting to be sorted. Safe to edit by hand.") {
        fails.push(format!("empty_file has no header comment: {}", q(&empty)));
    }
    if !empty.contains("local h = require(\"snips.helpers\")") {
        fails.push("empty_file lacks local h = require(\"snips.helpers\")".into());
    }
    let locals = empty.lines().find(|l| l.starts_with("local math")).unwrap_or("");
    for hname in ["math", "math_tab", "text", "suffix", "post", "pat", "vis"] {
        if !locals.contains(&format!("h.{hname}")) {
            fails.push(format!("empty_file locals line lacks h.{hname}: {locals:?}"));
        }
    }
    if !empty.contains("return h.group(\"Inbox\"") {
        fails.push("empty_file does not return h.group(\"Inbox\", …)".into());
    }
    // save every A-case draft into the inbox, one after the other
    let mut ids = vec![];
    let cases: Vec<Draft> = expected()["snippets"].as_array().unwrap().iter().map(|c| draft_of(&c["draft"])).collect();
    for (i, d) in cases.iter().enumerate() {
        match inbox::save(&paths, d, &format!("2026-10-07T14:{:02}", i % 60)) {
            Ok(id) => ids.push(id),
            Err(e) => fails.push(format!("inbox::save({:?}) failed: {e}", d.trigger)),
        }
    }
    n += 1;
    let uniq: std::collections::HashSet<_> = ids.iter().collect();
    if uniq.len() != ids.len() {
        fails.push(format!("inbox ids are not unique: {ids:?}"));
    }
    let text = fs::read_to_string(paths.inbox()).unwrap_or_default();
    match inbox::read(&paths) {
        Ok(entries) => {
            n += 1;
            if entries.len() != cases.len() {
                fails.push(format!("inbox::read gives {} entries after {} saves", entries.len(), cases.len()));
            }
            for (e, d) in entries.iter().zip(&cases) {
                n += 2;
                if d.kind != Kind::RawLua && e.trigger != d.trigger {
                    fails.push(format!("inbox entry {} trigger {:?}, expected {:?}", e.id, e.trigger, d.trigger));
                }
                if d.kind != Kind::RawLua && d.check.is_empty() {
                    match &e.draft {
                        Some(p) if strip(p) == strip(d) => {}
                        other => fails.push(format!("inbox entry {} reads back as {:?}\n  expected {:?}", e.id, other.as_ref().map(strip), strip(d))),
                    }
                }
                let marker = text.lines().nth(e.line.saturating_sub(1)).unwrap_or("");
                if !marker.contains(&format!("id={}", e.id)) || !marker.contains(&format!("made={}", e.made)) {
                    fails.push(format!("entry {} line {} is not its marker: {marker:?}", e.id, e.line));
                }
            }
            // replace one (edit) and remove one
            if let (Some(first), Some(second)) = (entries.first(), entries.get(1)) {
                n += 2;
                let mut d = cases[0].clone();
                d.id = Some(first.id.clone());
                d.output = vec![Piece::Text("\\dfrac{".into()), Piece::Tab { n: 1, default: String::new() }, Piece::Text("}{".into()), Piece::Tab { n: 2, default: String::new() }, Piece::Text("}".into())];
                let _ = inbox::save(&paths, &d, "2026-10-07T15:00");
                let _ = inbox::remove(&paths, &[second.id.clone()]);
                let after = inbox::read(&paths).unwrap_or_default();
                if after.len() != entries.len() - 1 {
                    fails.push(format!("after replace + remove: {} entries, expected {}", after.len(), entries.len() - 1));
                }
                if !after.iter().any(|e| e.id == first.id && e.source.contains("\\dfrac")) {
                    fails.push("replacing an entry by id did not replace it".into());
                }
            }
        }
        Err(e) => fails.push(format!("inbox::read failed: {e}")),
    }
    // the whole inbox loads in real ntex, and ff (edited to dfrac) expands from inbox.lua
    let got = ntex(&paths.snippets, &json!([
        {"name": "errors", "op": "errors"},
        {"name": "ff", "ft": "tex", "text": "$‸$", "steps": ["ff"]},
        {"name": "pct", "ft": "tex", "text": "$‸$", "steps": ["pct"]},
    ]));
    n += 3;
    if got[0]["errors"].as_object().map(|o| !o.is_empty()).unwrap_or(true) {
        fails.push(format!("ntex reports load errors with the studio's inbox.lua: {}\n--- inbox.lua ---\n{text}", got[0]["errors"]));
    }
    let ffb = &got[1]["steps"][0];
    if ffb["buffer"] != json!("$\\dfrac{‸}{}$") {
        fails.push(format!("after saving, ntex (fresh start) types ff as {} (expected $\\dfrac{{‸}}{{}}$)", ffb["buffer"]));
    }
    if got[2]["steps"][0]["buffer"] != json!("$50\\%‸$") {
        fails.push(format!("inbox pct gives {}", got[2]["steps"][0]["buffer"]));
    }
    // with_draft on the empty file loads too
    n += 1;
    let (t2, _id) = inbox::with_draft(&empty, &cases[0], "2026-10-07T14:32");
    let g = ntex(&paths.snippets, &json!([{"name": "chk", "op": "check", "file": "inbox_probe.lua", "text": t2}]));
    if g[0]["check"]["ok"] != json!(true) {
        fails.push(format!("with_draft(empty_file, ff) does not load: {}", g[0]["check"]["err"]));
    }
    report("inbox_file_roundtrip_and_loads", &fails, n);
}

#[test]
fn luaparse_splits_her_real_files() {
    let mut fails = vec![];
    let mut n = 0;
    let files = expected()["files"].as_object().unwrap();
    for (name, info) in files {
        n += 2;
        let text = fs::read_to_string(config_real().join("snippets/tex").join(name)).unwrap();
        let entries = luaparse::split_entries(&text);
        let want = info["n"].as_u64().unwrap() as usize;
        if entries.len() != want {
            fails.push(format!("split_entries({name}) = {} entries, expected {want} (counted by running the file with stub helpers)", entries.len()));
        }
        let trigs: Vec<Option<String>> = entries.iter().map(|(_, s)| luaparse::trigger_of(s)).collect();
        let want_t: Vec<Option<String>> = info["triggers"].as_array().unwrap().iter().map(|t| t.as_str().map(String::from)).collect();
        if trigs != want_t {
            fails.push(format!("trigger_of over split_entries({name})\n  expected {want_t:?}\n  actual   {trigs:?}"));
        }
    }
    report("luaparse_splits_her_real_files", &fails, n);
}

// ---------------------------------------------------------------- D: blocks

fn compile(dir: &Path, templates: &Path, body: &str) -> (bool, String, String) {
    let doc = format!(
        "\\documentclass{{article}}\n\\usepackage{{ntex-notes}}\n\\begin{{document}}\n\\section{{One}}\n\\begin{{theorem}}\\label{{thm:a}} T. \\end{{theorem}}\n{body}\n\\end{{document}}\n"
    );
    fs::write(dir.join("doc.tex"), doc).unwrap();
    let out = Command::new("tectonic")
        .arg("-Z").arg(format!("search-path={}", templates.display()))
        .args(["--keep-logs", "--outdir", "."])
        .arg("doc.tex")
        .current_dir(dir)
        .output()
        .expect("tectonic");
    let err = String::from_utf8_lossy(&out.stderr).lines().filter(|l| l.starts_with("error") || l.starts_with('!')).take(3).collect::<Vec<_>>().join(" | ");
    let mut text = String::new();
    if out.status.success() {
        let gs = Command::new("gs").args(["-q", "-dBATCH", "-dNOPAUSE", "-dSAFER", "-sDEVICE=txtwrite", "-o", "-", "doc.pdf"]).current_dir(dir).output();
        if let Ok(g) = gs {
            text = String::from_utf8_lossy(&g.stdout).to_string();
        }
    }
    (out.status.success(), err, text)
}
fn squash(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn d_blocks_compile_and_cref() {
    let mut fails = vec![];
    let mut n = 0;
    for b in expected()["blocks"].as_array().unwrap() {
        let id = b["id"].as_str().unwrap();
        let why = b["why"].as_str().unwrap();
        let block: BlockDraft = serde_json::from_value(b["block"].clone()).unwrap();
        let st = blocks::stanza(&block);
        for h in b["stanza_has"].as_array().unwrap() {
            n += 1;
            if !st.contains(h.as_str().unwrap()) {
                fails.push(format!("{id} ({why}): stanza lacks {h}\n{st}"));
            }
        }
        for h in b["stanza_lacks"].as_array().unwrap() {
            n += 1;
            if st.contains(h.as_str().unwrap()) {
                fails.push(format!("{id} ({why}): stanza should not contain {h}\n{st}"));
            }
        }
        // blocks::save into a temp copy of templates/tex, then compile a document with tectonic
        let paths = temp_paths(&format!("blk-{id}"));
        n += 1;
        if let Err(e) = blocks::save(&paths, &block) {
            fails.push(format!("{id}: blocks::save failed: {e}"));
            continue;
        }
        let notes = fs::read_to_string(paths.notes_sty()).unwrap();
        n += 1;
        let rp = notes.find("\\RequirePackage{ntex-blocks}");
        let ei = notes.rfind("\\endinput");
        if !(rp.is_some() && ei.is_some() && rp < ei) {
            fails.push(format!("{id}: ntex-notes.sty has no \\RequirePackage{{ntex-blocks}} before \\endinput"));
        }
        let work = paths.state.join("tex");
        fs::create_dir_all(&work).unwrap();
        let (ok, err, text) = compile(&work, &paths.templates_tex, b["doc"].as_str().unwrap());
        n += 1;
        if !ok {
            fails.push(format!("{id} ({why}): the document does not compile with the saved block: {err}\n--- stanza ---\n{st}"));
        } else {
            for h in b["pdf_has"].as_array().unwrap() {
                n += 1;
                if !squash(&text).contains(&squash(h.as_str().unwrap())) {
                    fails.push(format!("{id} ({why}): PDF text lacks {h:?}; it reads: {:?}", text.split_whitespace().collect::<Vec<_>>().join(" ")));
                }
            }
        }
        // blocks::read finds it again
        n += 1;
        match blocks::read(&paths) {
            Ok(list) if list.iter().any(|k| k.draft.env == block.env) => {}
            other => fails.push(format!("{id}: blocks::read does not give the saved block back: {other:?}")),
        }
        // the snippet, generated and typed in real ntex
        if let Some(se) = b.get("snippet_entry") {
            n += 1;
            match blocks::snippet(&block) {
                None => fails.push(format!("{id}: blocks::snippet = None for trigger {:?}", block.trigger)),
                Some(d) => {
                    let e = codegen::entry(&d);
                    let alt = se.as_str().unwrap().replace("{ line = \"start\", tab = true", "{ tab = true, line = \"start\"");
                    // her files open a multi-line body with `[[` + newline (Lua drops it): accepted
                    let alts = [se.as_str().unwrap().to_string(), alt.clone(), se.as_str().unwrap().replacen("[[", "[[\n", 1), alt.replacen("[[", "[[\n", 1)];
                    // a desc is allowed (it only feeds the reference page)
                    let mut e_nodesc = e.clone();
                    if let Some(i) = e_nodesc.find(", desc = \"") {
                        if let Some(j) = e_nodesc[i + 10..].find('"') {
                            e_nodesc.replace_range(i..i + 10 + j + 1, "");
                        }
                    }
                    if !alts.contains(&e) && !alts.contains(&e_nodesc) {
                        fails.push(format!("{id}: block snippet entry\n  expected {}\n  actual   {}", q(se.as_str().unwrap()), q(&e)));
                    }
                    let ty = &b["snippet_type"];
                    let keys = ty["keys"].clone();
                    let g = ntex(&paths.snippets, &json!([{"name": "blk", "entry": e, "ft": "tex", "text": "‸", "steps": keys}]));
                    for (k, want) in ty["buffers"].as_array().unwrap().iter().enumerate() {
                        n += 1;
                        if &g[0]["steps"][k]["buffer"] != want {
                            fails.push(format!("{id}: typing {} gives {} (expected {want})", ty["keys"][k], g[0]["steps"][k]["buffer"]));
                        }
                    }
                }
            }
        }
        // remove → gone, file still compiles
        n += 1;
        let _ = blocks::remove(&paths, &block.env);
        if let Ok(list) = blocks::read(&paths) {
            if list.iter().any(|k| k.draft.env == block.env) {
                fails.push(format!("{id}: blocks::remove left it in the file"));
            }
        }
    }
    for c in expected()["block_checks"].as_array().unwrap() {
        n += 1;
        let block: BlockDraft = serde_json::from_value(c["block"].clone()).unwrap();
        let f = blocks::check(&block, &[], &known());
        let has = f.iter().any(|x| x.severity == Severity::Error);
        if has != c["has_error"].as_bool().unwrap() {
            fails.push(format!("blocks::check: {}: has Error = {has}; findings {f:?}", c["why"].as_str().unwrap()));
        }
    }
    for p in expected()["env_from_title"].as_array().unwrap() {
        n += 1;
        let got = blocks::env_from_title(p[0].as_str().unwrap());
        if got != p[1].as_str().unwrap() {
            fails.push(format!("env_from_title({}) = {got:?}, expected {}", p[0], p[1]));
        }
    }
    report("d_blocks_compile_and_cref", &fails, n);
}

fn wcag(a: Rgb, b: Rgb) -> f32 {
    fn lin(c: u8) -> f64 {
        let c = c as f64 / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    }
    fn l(x: Rgb) -> f64 {
        0.2126 * lin(x.0) + 0.7152 * lin(x.1) + 0.0722 * lin(x.2)
    }
    let (x, y) = (l(a), l(b));
    let (hi, lo) = if x > y { (x, y) } else { (y, x) };
    ((hi + 0.05) / (lo + 0.05)) as f32
}
fn rgb(h: &str) -> Rgb {
    let p = |i| u8::from_str_radix(&h[i..i + 2], 16).unwrap();
    Rgb(p(0), p(2), p(4))
}

#[test]
fn d_colour() {
    let mut fails = vec![];
    let mut n = 0;
    let c = &expected()["colour"];
    let pal = colour::palette();
    for (name, tones) in c["palette"].as_object().unwrap() {
        n += 1;
        let t = tones.as_array().unwrap();
        let want = Tones { rule: rgb(t[0].as_str().unwrap()), back: rgb(t[1].as_str().unwrap()), head: rgb(t[2].as_str().unwrap()) };
        match pal.iter().find(|(n, _)| n == name) {
            Some((_, got)) if *got == want => {}
            other => fails.push(format!("palette {name}: {other:?}, expected {want:?} (ntex-notes.sty)")),
        }
        n += 1;
        if colour::tones(&BlockColour::Palette(name.clone())) != want {
            fails.push(format!("tones(Palette({name})) != the sty's"));
        }
    }
    for t in c["contrast"].as_array().unwrap() {
        n += 1;
        let got = colour::contrast(rgb(t[0].as_str().unwrap()), rgb(t[1].as_str().unwrap()));
        if (got - t[2].as_f64().unwrap() as f32).abs() > 0.05 {
            fails.push(format!("contrast({}, {}) = {got}, expected {}", t[0], t[1], t[2]));
        }
    }
    let ink = rgb(c["ink"].as_str().unwrap());
    for h in c["hue_sweep"].as_array().unwrap() {
        n += 1;
        let h = h.as_f64().unwrap() as f32;
        let t = colour::tones_for_hue(h);
        let (body, head) = (wcag(ink, t.back), wcag(t.head, t.back));
        if body < 7.0 || head < 4.5 {
            fails.push(format!("tones_for_hue({h}) = {t:?}: ink on back {body:.2} (≥7), head on back {head:.2} (≥4.5)"));
        }
        n += 1;
        if colour::hex(t.rule).len() != 6 {
            fails.push(format!("hex({:?}) = {:?}, expected 6 hex digits (\\definecolor{{…}}{{HTML}}{{…}})", t.rule, colour::hex(t.rule)));
        }
    }
    report("d_colour", &fails, n);
}

// ---------------------------------------------------------------- E: apply

fn snapshot(dir: &Path) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "lua").unwrap_or(false))
        .map(|e| (e.file_name().to_string_lossy().to_string(), fs::read_to_string(e.path()).unwrap()))
        .collect();
    v.sort();
    v
}

fn ntex_check(path: &Path, text: &str) -> Result<(), String> {
    // ntex's own loader check, on a scratch copy holding `text` under the same name
    let dir = tmpdir("chk");
    let snips = dir.join("snippets");
    fs::create_dir_all(snips.join("tex")).unwrap();
    let g = ntex(&snips, &json!([{"name": "c", "op": "check", "file": path.file_name().unwrap().to_string_lossy(), "text": text}]));
    let _ = fs::remove_dir_all(&dir);
    if g[0]["check"]["ok"] == json!(true) { Ok(()) } else { Err(g[0]["check"]["err"].to_string()) }
}

#[test]
fn e_apply_sort_plan() {
    let mut fails = vec![];
    let mut n = 0;
    let a = &expected()["apply"];
    let plan: SortPlan = serde_json::from_value(a["plan"].clone()).unwrap();
    let tex = |p: &Paths| p.snippets.join("tex");

    // E1–E5: a successful apply
    let paths = temp_paths("apply");
    fs::write(paths.inbox(), a["inbox"].as_str().unwrap()).unwrap();
    let before = snapshot(&tex(&paths));
    match apply::plan_files(&paths, &plan) {
        Err(e) => fails.push(format!("plan_files failed: {e}")),
        Ok((files, outcome)) => {
            n += 1;
            let moved: std::collections::BTreeMap<String, String> = outcome.moved.iter().cloned().collect();
            let want: std::collections::BTreeMap<String, String> = serde_json::from_value(a["expect"]["moved"].clone()).unwrap();
            if moved != want {
                fails.push(format!("plan_files moved {moved:?}\n  expected {want:?}"));
            }
            n += 1;
            if before != snapshot(&tex(&paths)) {
                fails.push("plan_files wrote files (it must not)".into());
            }
            for (p, t) in &files {
                n += 1;
                if let Err(e) = ntex_check(p, t) {
                    fails.push(format!("plan_files: {} would not load in ntex: {e}\n--- text ---\n{t}", p.display()));
                }
            }
        }
    }
    let mut check = |p: &Path, t: &str| ntex_check(p, t);
    match apply::apply(&paths, &plan, "2026-10-07T15-00", &mut check) {
        Err(e) => fails.push(format!("apply failed on a good plan: {e}")),
        Ok(outcome) => {
            let files = snapshot(&tex(&paths));
            let get = |name: &str| files.iter().find(|(n, _)| n == name).map(|(_, t)| t.clone()).unwrap_or_default();
            for (file, order) in a["expect"]["order"].as_object().unwrap() {
                n += 1;
                let t = get(file);
                let trigs: Vec<String> = luaparse::split_entries(&t).iter().filter_map(|(_, s)| luaparse::trigger_of(s)).collect();
                let want: Vec<String> = order.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect();
                let pos: Vec<Option<usize>> = want.iter().map(|w| trigs.iter().position(|x| x == w)).collect();
                let ok = pos.iter().all(|p| p.is_some()) && pos.windows(2).all(|w| w[1] == w[0].map(|x| x + 1));
                let last_ok = want.len() > 1 || pos[0] == Some(trigs.len().saturating_sub(1));
                if !ok || !last_ok {
                    fails.push(format!("{file}: expected {want:?} adjacent in this order{}; triggers are {trigs:?}", if want.len() == 1 { " (at the end)" } else { "" }));
                }
            }
            for (file, title) in a["expect"]["new_file_title"].as_object().unwrap() {
                n += 1;
                if !get(file).contains(&format!("h.group({}", title)) {
                    fails.push(format!("{file} does not return h.group({title}, …):\n{}", get(file)));
                }
            }
            n += 1;
            let left: Vec<String> = luaparse::split_entries(&get("inbox.lua")).iter().filter_map(|(_, s)| luaparse::trigger_of(s)).collect();
            let keep: Vec<String> = serde_json::from_value(a["expect"]["inbox_keeps"].clone()).unwrap();
            if left != keep {
                fails.push(format!("inbox after sorting holds {left:?}, expected {keep:?}"));
            }
            // E2 + E5: start a fresh ntex on the sorted files, no load errors, type each
            let mut cs = vec![json!({"name": "errors", "op": "errors"})];
            for t in a["expect"]["type"].as_array().unwrap() {
                cs.push(json!({"name": t["keys"], "ft": t["ft"], "text": t["text"], "steps": [t["keys"]]}));
            }
            let g = ntex(&paths.snippets, &Value::Array(cs));
            n += 1;
            if g[0]["errors"].as_object().map(|o| !o.is_empty()).unwrap_or(true) {
                fails.push(format!("after apply, ntex reports load errors: {}", g[0]["errors"]));
            }
            for (t, r) in a["expect"]["type"].as_array().unwrap().iter().zip(&g[1..]) {
                n += 1;
                if r["steps"][0]["buffer"] != t["buffer"] {
                    fails.push(format!("after apply, typing {} gives {} (expected {})", t["keys"], r["steps"][0]["buffer"], t["buffer"]));
                }
            }
            // E7: undo restores byte for byte
            n += 1;
            match apply::undo(&outcome.backup) {
                Err(e) => fails.push(format!("undo failed: {e}")),
                Ok(()) => {
                    let now = snapshot(&tex(&paths));
                    if now != before {
                        let diff: Vec<&String> = now.iter().filter(|x| !before.contains(x)).map(|x| &x.0).chain(before.iter().filter(|x| !now.contains(x)).map(|x| &x.0)).collect();
                        fails.push(format!("undo did not restore every file byte for byte; differing: {diff:?}"));
                    }
                }
            }
        }
    }

    // E6: a failing check restores everything
    let paths = temp_paths("apply-fail");
    fs::write(paths.inbox(), a["inbox"].as_str().unwrap()).unwrap();
    let before = snapshot(&tex(&paths));
    let mut bad = |p: &Path, t: &str| {
        if p.file_name().map(|f| f == "operations.lua").unwrap_or(false) { Err("operations.lua:12: boom (adversary)".to_string()) } else { ntex_check(p, t) }
    };
    n += 2;
    let r = apply::apply(&paths, &plan, "2026-10-07T15-01", &mut bad);
    if r.is_ok() {
        fails.push(format!("apply returned Ok although the check failed for operations.lua: {r:?}"));
    }
    if snapshot(&tex(&paths)) != before {
        fails.push("after a failed check, the files are not all restored byte for byte".into());
    }
    report("e_apply_sort_plan", &fails, n);
}

// ---------------------------------------------------------------- X: probes beyond the spec tables

fn pd(trigger: &str, out: &str) -> Draft {
    Draft { trigger: trigger.into(), output: vec![Piece::Text(out.into())], ..Draft::default() }
}

/// Bodies and fields that break naive Lua writers / splitters, saved into the
/// inbox together, must load, read back and expand.
#[test]
fn x_hostile_text_roundtrips() {
    let mut fails = vec![];
    let mut n = 0;
    let paths = temp_paths("hostile");
    let mut ds = vec![
        pd("qdd", "a--b"),                       // a Lua comment marker inside the string
        pd("qcl", "}),"),                        // looks like the end of the list
        pd("qqt", "\"q\", 1)"),                  // quotes, comma, paren
        pd("qmk", "-- studio: id=fake made=x"),  // a marker spoof inside a body
        pd("qbr", "]]"),                         // only ]]
        pd("qbs", "\\\\"),                       // a LaTeX line break
    ];
    let mut multi = pd("qml", "x\n-- studio: id=fake2 made=y\n  math(\"zz\", [[\\zeta]], \"yours\"),\ny");
    multi.context = Context::Text;
    multi.fires = Fires::Tab;
    ds.push(multi);
    let mut d = pd("qds", "\\Delta");
    d.desc = "line one\nline \"two\" ]] \\ end".into();
    d.eg = "q\"ds".into();
    ds.push(d);
    let mut d = pd("qck", "\\kappa");
    d.check = "local s = [[x]] return line ~= s".into();
    ds.push(d);
    let mut d = pd("]]q", "\\rho");
    d.trigger = "]]q".into();
    ds.push(d);
    for (i, d) in ds.iter().enumerate() {
        if let Err(e) = inbox::save(&paths, d, &format!("2026-10-07T16:{i:02}")) {
            fails.push(format!("save {:?}: {e}", d.trigger));
        }
    }
    let entries = inbox::read(&paths).unwrap_or_default();
    n += 1;
    if entries.len() != ds.len() {
        fails.push(format!("inbox::read gives {} entries for {} saved (a body fooled the splitter?): {:?}", entries.len(), ds.len(), entries.iter().map(|e| (&e.id, &e.trigger)).collect::<Vec<_>>()));
    }
    for d in &ds {
        n += 1;
        match entries.iter().find(|e| e.trigger == d.trigger) {
            None => fails.push(format!("no inbox entry with trigger {:?}", d.trigger)),
            Some(e) => {
                if d.check.is_empty() {
                    if e.draft.as_ref().map(strip) != Some(strip(d)) {
                        fails.push(format!("entry {:?} reads back as {:?}", d.trigger, e.draft.as_ref().map(strip)));
                    }
                }
            }
        }
    }
    let text = fs::read_to_string(paths.inbox()).unwrap_or_default();
    let g = ntex(&paths.snippets, &json!([
        {"name": "errors", "op": "errors"},
        {"name": "qdd", "ft": "tex", "text": "$‸$", "steps": ["qdd"]},
        {"name": "qcl", "ft": "tex", "text": "$‸$", "steps": ["qcl"]},
        {"name": "qqt", "ft": "tex", "text": "$‸$", "steps": ["qqt"]},
        {"name": "qbr", "ft": "tex", "text": "$‸$", "steps": ["qbr"]},
        {"name": "qbs", "ft": "tex", "text": "$‸$", "steps": ["qbs"]},
        {"name": "rho", "ft": "tex", "text": "$a ‸$", "steps": ["]]q"]},
        {"name": "zz-not-loaded", "ft": "tex", "text": "$‸$", "steps": ["zz"]},
    ]));
    n += 1;
    if g[0]["errors"].as_object().map(|o| !o.is_empty()).unwrap_or(true) {
        fails.push(format!("inbox.lua with hostile bodies does not load: {}\n--- inbox.lua ---\n{text}", g[0]["errors"]));
    }
    let want = ["$a--b‸$", "$}),‸$", "$\"q\", 1)‸$", "$]]‸$", "$\\\\‸$", "$a \\rho‸$", "$zz‸$"];
    for (k, w) in want.iter().enumerate() {
        n += 1;
        if g[k + 1]["steps"][0]["buffer"] != json!(w) {
            fails.push(format!("{}: typed gives {} (expected {w})", g[k + 1]["name"], g[k + 1]["steps"][0]["buffer"]));
        }
    }
    report("x_hostile_text_roundtrips", &fails, n);
}

/// The sort plan comes from a model: its file names are untrusted.
#[test]
fn x_plan_cannot_escape_the_snippets_folder() {
    let paths = temp_paths("escape");
    let a = &expected()["apply"];
    fs::write(paths.inbox(), a["inbox"].as_str().unwrap()).unwrap();
    let mut fails = vec![];
    let evil = ["../../evil.lua", "/tmp/ntex-adversary-evil.lua", "sub/dir.lua", "evil.txt", ".hidden.lua", "inbox.lua"];
    for f in evil {
        let plan = SortPlan { moves: vec![SortMove { id: "stay-1".into(), file: f.into(), after_trigger: None, new_title: Some("Evil".into()), reason: "x".into() }], notes: String::new() };
        let before = snapshot(&paths.snippets.join("tex"));
        let mut ok = |_: &Path, _: &str| Ok(());
        let r = apply::apply(&paths, &plan, "2026-10-07T17-00", &mut ok);
        let escaped = paths.snippets.join("tex").join(f);
        let outside = [paths.config.join("evil.lua"), paths.snippets.join("evil.lua"), PathBuf::from("/tmp/ntex-adversary-evil.lua"), paths.snippets.join("tex/sub/dir.lua")];
        for o in &outside {
            if o.exists() {
                fails.push(format!("plan file {f:?} wrote {}", o.display()));
                let _ = fs::remove_file(o);
            }
        }
        if let Ok(out) = &r {
            if out.moved.iter().any(|(_, file)| file == f) && (f.contains('/') || !f.ends_with(".lua") || f.starts_with('.') || f == "inbox.lua") {
                fails.push(format!("plan file {f:?} was accepted ({out:?}); expected it kept in the inbox with a reason"));
                let _ = apply::undo(&out.backup);
            }
        }
        let _ = escaped;
        let after = snapshot(&paths.snippets.join("tex"));
        if r.is_err() && after != before {
            fails.push(format!("plan file {f:?}: apply failed and left files changed"));
        }
    }
    report("x_plan_cannot_escape_the_snippets_folder", &fails, evil.len());
}

#[test]
fn x_self_is_not_a_duplicate() {
    let mut k = known();
    k.push(KnownSnippet { trigger: "ff".into(), pattern: None, kind: "math".into(), where_: "math".into(), how: "auto".into(), body: Some("\\frac{<>}{<>}".into()), file: "inbox.lua".into(), line: Some(7), origin: "yours".into(), group: Some("Inbox".into()), line_rule: None, ft: None, priority: 0, desc: None });
    let mut d = draft_of(&expected()["snippets"][0]["draft"]);
    d.id = Some("ff-1432".into());
    let f = infer::check(&d, &k);
    let mut fails = vec![];
    if f.iter().any(|x| x.code == "duplicate" || x.code == "shadows" || x.code == "shadowed-by") {
        fails.push(format!("editing inbox entry ff-1432 (ff), its own loaded copy in inbox.lua is reported as a conflict: {f:?}"));
    }
    report("x_self_is_not_a_duplicate", &fails, 1);
}
