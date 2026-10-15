//! Real LuaSnip fields, entirely headless, over private snippet copies.
mod common;
use common::*;
use ntex_studio_io::bridge::{Bridge, FieldState};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

const T: Duration = Duration::from_secs(60);

fn answer<A>(rx: Receiver<Result<A, String>>) -> A {
    rx.recv_timeout(T).expect("bridge answer").unwrap_or_else(|e| panic!("bridge: {e}"))
}

fn caret(s: &FieldState) -> String {
    let mut text = s.text.clone();
    text.insert_str(s.cursor, "‸");
    text
}

fn keys(b: &Bridge, id: u64, k: &str) -> FieldState {
    answer(b.field_keys(id, k))
}

fn field_bridge(t: &TempDir) -> Bridge {
    let mut paths = temp_paths(t);
    // Instrument only a temporary server copy. The real config is read-only.
    let script = std::fs::read_to_string(paths.config.join("studio/bridge/server.lua")).unwrap();
    let instrument = r#"
lua([==[
  local writes = 0
  vim.api.nvim_create_autocmd("BufWritePre", {
    callback = function(ev)
      if vim.api.nvim_buf_get_name(ev.buf):match("/field%-%d+%.tex$") then
        writes = writes + 1
      end
    end,
  })
  local state = ntex_bridge.field_state
  ntex_bridge.field_state = function(id)
    local buf = ntex_bridge.fields[id].buf
    assert(vim.bo[buf].buftype == "nofile", "field must be scratch")
    assert(vim.bo[buf].filetype == "tex", "field must be tex")
    assert(not vim.bo[buf].swapfile and not vim.bo[buf].undofile)
    assert(vim.env.TEXNVIM_NO_LSP == "1" and vim.env.TEXNVIM_NO_COMPILE == "1")
    assert(vim.env.TEXNVIM_NO_VIEWER == "1")
    assert(#vim.lsp.get_clients({ bufnr = buf }) == 0, "field has an LSP")
    local ok, hooks = pcall(vim.api.nvim_get_autocmds, { group = "texnvim_live_pdf" })
    assert(not ok or #hooks == 0, "livepdf enabled")
    assert(writes == 0, "BufWritePre fired")
    assert(not vim.uv.fs_stat(vim.api.nvim_buf_get_name(buf)), "field autosaved")
    local mode = vim.fn.mode()
    assert(mode == "i" or mode == "s", "field left insert/select: " .. mode)
    return state(id)
  end
]==])
"#;
    let needle = "---------------------------------------------------------------------------\n-- typing";
    assert!(script.contains(needle));
    let config = t.path().join("config");
    std::fs::create_dir_all(config.join("studio/bridge")).unwrap();
    std::fs::write(config.join("studio/bridge/server.lua"), script.replacen(needle, &format!("{instrument}\n{needle}"), 1)).unwrap();
    paths.config = config;
    let b = Bridge::start(&paths).unwrap();
    assert!(b.wait_ready(T), "headless bridge never ready");
    b
}

fn gone(b: Bridge, mut seen: Vec<u32>) {
    seen.extend(b.pids());
    let dir = b.private_dir().unwrap();
    drop(b);
    let deadline = Instant::now() + Duration::from_secs(3);
    while seen.iter().any(|p| alive(*p)) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    for p in seen {
        assert!(!alive(p), "process {p} survived Drop");
        assert!(group_members(p).is_empty(), "pgrep: process group {p} survived Drop");
    }
    assert!(!dir.exists(), "private scratch folder survived Drop");
    let out = std::process::Command::new("pgrep").args(["-f", &dir.to_string_lossy()]).output().unwrap();
    assert!(out.stdout.is_empty(), "pgrep: private bridge process survived Drop");
}

#[test]
fn real_snippet_fields() {
    let t = TempDir::new("fields");
    let b = field_bridge(&t);
    let (id, st) = answer(b.field_open("", 0));
    assert_eq!(st, FieldState::default());
    assert_eq!(caret(&keys(&b, id, "m")), "m‸");
    assert_eq!(caret(&keys(&b, id, "k")), "$‸$");
    assert_eq!(caret(&keys(&b, id, "Z")), "$Z‸$");
    assert_eq!(caret(&keys(&b, id, "Z")), "$\\mathbb{Z}‸$");

    answer(b.field_set(id, "$$", 1));
    assert_eq!(caret(&keys(&b, id, "/")), "$/‸$");
    let st = keys(&b, id, "/");
    assert_eq!(caret(&st), "$\\frac{‸}{}$");
    assert!(st.in_snippet);
    assert_eq!(caret(&keys(&b, id, "a")), "$\\frac{a‸}{}$");
    assert_eq!(caret(&keys(&b, id, "<Tab>")), "$\\frac{a}{‸}$");
    assert_eq!(caret(&keys(&b, id, "b")), "$\\frac{a}{b‸}$");
    assert_eq!(caret(&keys(&b, id, "<Tab>")), "$\\frac{a}{b}‸$");

    answer(b.field_set(id, "", 0));
    assert_eq!(caret(&keys(&b, id, "plain ZZ")), "plain ZZ‸");
    answer(b.field_set(id, "$$", 1));
    let st = keys(&b, id, "<BS>");
    // The user's prefs determine the pair-deletion behavior.
    let prefs = std::fs::read_to_string(real_config().join("prefs.lua")).unwrap_or_default();
    if !prefs.contains("auto_delete_dollar = false") {
        assert_eq!(caret(&st), "‸");
    }
    assert_eq!(caret(&answer(b.field_set(id, "mk $ZZ$ //", 6))), "mk $ZZ‸$ //");
    assert!(!keys(&b, id, "").in_snippet);
    assert!(b.field_set(id, "two\nlines", 0).recv_timeout(T).unwrap().is_err());
    assert_eq!(keys(&b, id, "").text, "mk $ZZ$ //");
    let st = answer(b.field_set(id, "é 🐟", 999));
    assert_eq!(caret(&st), "é 🐟‸");
    assert_eq!(caret(&answer(b.field_set(id, "é 🐟", 5))), "é ‸🐟");
    assert_eq!(caret(&keys(&b, id, "<lt>")), "é <‸🐟");
    answer(b.field_set(id, "$$", 1));
    assert!(keys(&b, id, "//").in_snippet);
    let st = keys(&b, id, "<Esc>");
    assert!(!st.in_snippet);
    assert!(!keys(&b, id, "x").in_snippet);

    // Include enough idle time for livepdf's normal debounce to have fired.
    std::thread::sleep(Duration::from_millis(1000));
    keys(&b, id, "");
    b.field_close(id);
    assert!(b.field_keys(id, "x").recv_timeout(T).unwrap().is_err());
    gone(b, vec![]);
}

#[test]
fn independent_fields_and_restart() {
    let t = TempDir::new("field-restart");
    let b = field_bridge(&t);
    let seen = b.pids();
    let (a, _) = answer(b.field_open("$$", 1));
    let st = keys(&b, a, "//a");
    assert!(st.in_snippet);
    let (z, _) = answer(b.field_open("text ", 5));
    assert_eq!(caret(&keys(&b, z, "ZZ")), "text ZZ‸");
    assert_eq!(caret(&keys(&b, a, "<Tab>b")), "$\\frac{a}{b‸}$");
    assert_eq!(caret(&keys(&b, z, "!")), "text ZZ!‸");

    // Restart restores each last acknowledged text and insert byte offset.
    answer(b.field_set(a, "$ $", 2));
    answer(b.field_set(z, "hello", 2));
    b.kill_for_test();
    assert_eq!(caret(&keys(&b, a, "ZZ")), "$ \\mathbb{Z}‸$");
    assert_eq!(caret(&keys(&b, z, "X")), "heX‸llo");
    assert_eq!(b.starts(), 2);
    // Pipelined requests must answer in key order.
    let r1 = b.field_keys(z, "1");
    let r2 = b.field_keys(z, "2");
    let r3 = b.field_keys(z, "3");
    assert_eq!(caret(&answer(r1)), "heX1‸llo");
    assert_eq!(caret(&answer(r2)), "heX12‸llo");
    assert_eq!(caret(&answer(r3)), "heX123‸llo");
    gone(b, seen);
}

#[test]
fn selected_defaults_stay_independent() {
    let t = TempDir::new("field-select");
    let paths = temp_paths(&t);
    // A default tab stop in the temp snippets only.
    std::fs::write(paths.snippets.join("tex/zz_field_test.lua"), r#"local h = require("snips.helpers")
return h.group("Field test", {h.math("zqj", [[\sqrt{<:é🐟>}]], "yours")})
"#).unwrap();
    let b = Bridge::start(&paths).unwrap();
    assert!(b.wait_ready(T));
    let (a, _) = answer(b.field_open("$$", 1));
    let st = keys(&b, a, "zqj");
    println!("selected default: {st:?}");
    assert_eq!(st.text, "$\\sqrt{é🐟}$");
    assert_eq!(st.selected, Some((7, 13)));
    assert!(st.in_snippet);
    let (other, _) = answer(b.field_open("other", 5));
    keys(&b, other, "!");
    assert_eq!(keys(&b, a, "").selected, Some((7, 13)));
    assert_eq!(caret(&keys(&b, a, "x")), "$\\sqrt{x‸}$");
    assert_eq!(keys(&b, other, "").text, "other!");
    gone(b, vec![]);
}

#[test]
fn a_field_luasnip_error_heals_before_the_next_request() {
    let t = TempDir::new("field-heal");
    let paths = temp_paths(&t);
    std::fs::write(paths.snippets.join("tex/zz_field_test.lua"), r#"local h = require("snips.helpers")
return h.group("Field test", {h.math("zqe", [[\zeta]], "yours", {
    check = function() error("field self-heal test") end,
})})
"#).unwrap();
    let b = Bridge::start(&paths).unwrap();
    assert!(b.wait_ready(T));
    let seen = b.pids();
    let (id, _) = answer(b.field_open("$$", 1));
    let failed = b.field_keys(id, "zqe");
    let next = b.field_set(id, "$$", 1);
    let error = failed.recv_timeout(T).unwrap().expect_err("condition should throw");
    assert!(error.contains("field self-heal test"), "{error}");
    assert_eq!(caret(&answer(next)), "$‸$");
    assert_eq!(caret(&keys(&b, id, "ZZ")), "$\\mathbb{Z}‸$");
    assert_eq!(b.starts(), 2, "field errors must use the planned-restart path");
    let mut seen = seen;
    seen.extend(b.pids());
    let error = b.field_keys(id, "<CR>").recv_timeout(T).unwrap().expect_err("fields must stay on one line");
    assert!(error.contains("one line"), "{error}");
    assert_eq!(caret(&keys(&b, id, "")), "$\\mathbb{Z}‸$");
    assert_eq!(b.starts(), 3, "invalid field state must recover");
    gone(b, seen);
}

#[test]
fn warm_key_latency() {
    let t = TempDir::new("field-latency");
    let paths = temp_paths(&t);
    let b = Bridge::start(&paths).unwrap();
    assert!(b.wait_ready(T));
    let (id, _) = answer(b.field_open("", 0));
    for _ in 0..20 { keys(&b, id, "x"); }
    let mut ms = Vec::new();
    for _ in 0..200 {
        let start = Instant::now();
        keys(&b, id, "x");
        ms.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    ms.sort_by(f64::total_cmp);
    let median = (ms[99] + ms[100]) / 2.0;
    let p95 = ms[189];
    println!("warm field_keys, 200 individual round trips: median {median:.3} ms; p95 {p95:.3} ms");
    // Include real expansions, tab-stop jumps, and Escape in another 200
    // individual requests. Reset the field outside the timed interval.
    let sequence = ["Z", "Z", " ", "/", "/", "a", "<Tab>", "b", "<Tab>", "<Esc>"];
    answer(b.field_set(id, "$$", 1));
    for key in sequence { keys(&b, id, key); }
    let mut mixed = Vec::new();
    for _ in 0..20 {
        answer(b.field_set(id, "$$", 1));
        for key in sequence {
            let start = Instant::now();
            keys(&b, id, key);
            mixed.push(start.elapsed().as_secs_f64() * 1000.0);
        }
    }
    mixed.sort_by(f64::total_cmp);
    let mixed_median = (mixed[99] + mixed[100]) / 2.0;
    let mixed_p95 = mixed[189];
    println!("warm field_keys with expansions/jumps/Escape, 200 individual round trips: median {mixed_median:.3} ms; p95 {mixed_p95:.3} ms");
    gone(b, vec![]);
    assert!(p95 <= 8.0, "warm p95 {p95:.3} ms exceeds 8 ms target (median {median:.3} ms)");
    assert!(mixed_p95 <= 8.0, "warm snippet p95 {mixed_p95:.3} ms exceeds 8 ms target (median {mixed_median:.3} ms)");
}
