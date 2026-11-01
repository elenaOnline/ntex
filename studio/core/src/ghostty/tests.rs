use super::screen::{self, Paint, Target};
use super::*;

/// Her ~/.config/ghostty/config's look lines (2026-10-20).
const HER_CONFIG: &str = "font-family = JuliaMono
background = #ffffff
foreground = #000000
cursor-color = #007acc
cursor-text = #ffffff
selection-background = #bfdbfe
selection-foreground = #000000
palette = 0=#000000
palette = 1=#aa3731
palette = 15=#ffffff
window-padding-x = 4
window-padding-y = 12
window-padding-balance = true
adjust-cell-height = 10%
unfocused-split-opacity = 1
macos-option-as-alt = true
config-file = ?ntex-look.ghostty
";

/// What `ghostty +show-config` prints for it (theme-resolved, style families filled in).
const HER_SHOW: &str = "font-family = JuliaMono
font-family-bold = JuliaMono
font-family-italic = JuliaMono
font-family-bold-italic = JuliaMono
adjust-cell-height = 10.000000000000009%
theme = Breadog
background = #ffffff
foreground = #000000
selection-background = #bfdbfe
selection-foreground = #000000
palette = 1=#aa3731
cursor-color = #007acc
window-padding-x = 4
window-padding-y = 12
window-padding-balance = true
unfocused-split-opacity = 1
macos-option-as-alt = true
";

fn d() -> Defaults {
    Defaults::bundled()
}

#[test]
fn catalog_covers_every_ghostty_key() {
    let d = d();
    assert!(d.keys.len() > 150, "parsed {} keys", d.keys.len());
    for k in &d.keys {
        assert!(
            k == "palette" || catalog::find(k).is_some() || catalog::NON_LOOK.contains(&k.as_str()),
            "{k}: new in this Ghostty? add it to catalog::LOOK or NON_LOOK"
        );
    }
    for o in catalog::LOOK {
        assert!(d.keys.iter().any(|k| k == o.key), "{} is not a Ghostty 1.3 key", o.key);
        assert!(!catalog::NON_LOOK.contains(&o.key), "{} is in both lists", o.key);
    }
    assert!(d.unknown_look_keys().is_empty(), "{:?}", d.unknown_look_keys());
}

#[test]
fn defaults_and_docs() {
    let d = d();
    assert_eq!(d.get("font-size"), vec!["13"]);
    assert_eq!(d.get("cursor-style"), vec!["block"]);
    assert_eq!(d.get("font-family"), Vec::<String>::new());
    assert!(d.get("palette:1")[0].starts_with('#'));
    assert!(d.docs["cursor-style"].contains("block_hollow"));
    // a key with no docs of its own shares the one before it
    assert_eq!(d.docs["font-family-bold"], d.docs["font-family"]);
}

#[test]
fn her_effective_look() {
    let d = d();
    let l = TermLook::from_show_config(HER_SHOW, &d);
    assert_eq!(l.one("background", &d), "#ffffff");
    assert_eq!(l.one("adjust-cell-height", &d), "10%");
    assert_eq!(l.get("font-family", &d), vec!["JuliaMono"]);
    assert!(!l.edited("font-family-bold"), "derived style families are dropped");
    assert!(!l.edited("macos-option-as-alt"), "not a look key");
    assert_eq!(l.one("theme", &d), "Breadog");
    assert_eq!(l.color("palette:1", &d), Some(0xaa3731));
}

#[test]
fn file_round_trip() {
    let d = d();
    let mut l = TermLook::from_show_config(HER_SHOW, &d);
    l.reset("theme");
    l.put("font-family", vec!["Menlo".into(), "JuliaMono".into()], &d);
    l.put1("cursor-style", "bar", &d);
    l.put1("window-padding-x", "2,6", &d);
    l.put1("palette:200", "#123456", &d);
    let pinned = pinned_keys(&[HER_CONFIG]);
    let text = l.to_file("Ghostty today (edited)", &d, &pinned);
    assert!(text.lines().any(|x| x == "theme = \"\""), "{text}");
    assert!(text.contains("font-family = \"\"\nfont-family = Menlo\nfont-family = JuliaMono\n"), "{text}");
    assert!(text.contains("palette = 200=#123456"));
    assert!(text.contains("window-padding-x = 2,6"));
    assert!(!text.contains("macos-option-as-alt"));
    assert_eq!(TermLook::from_file(&text, &d), l);
}

#[test]
fn pinned_keys_win_even_at_default() {
    let d = d();
    // the look wants Ghostty's own cell height and cursor color: the file
    // must still say so, or her config's 10% and #007acc would show
    let l = TermLook::default();
    let text = l.to_file("Ghostty default", &d, &pinned_keys(&[HER_CONFIG]));
    assert!(text.contains("adjust-cell-height = \"\"\n"), "{text}");
    assert!(text.contains("cursor-color = \"\"\n"), "{text}");
    assert!(text.contains(&format!("palette = 1={}\n", d.get("palette:1")[0])), "{text}");
    assert!(!text.contains("palette = 2="), "only the entries her config sets");
    assert_eq!(TermLook::from_file(&text, &d), l);
}

#[test]
fn numbers_are_short() {
    assert_eq!(norm_number("10.000000000000009%"), "10%");
    assert_eq!(norm_number("1.0"), "1");
    assert_eq!(norm_number("0.95"), "0.95");
    assert_eq!(norm_number("-2"), "-2");
    assert_eq!(norm_number("750ms"), "750ms");
}

const BREADOG: &str = "palette = 0=#362c24\npalette = 1=#b10b00\npalette = 15=#eae1da\nbackground = #f1ebe6\nforeground = #362c24\ncursor-color = #362c24\ncursor-text = #f1ebe6\nselection-background = #362c24\nselection-foreground = #f1ebe6\n";

#[test]
fn themes() {
    let d = d();
    let t = parse_theme("Breadog", BREADOG, &d);
    assert_eq!(t.bg, 0xf1ebe6);
    assert_eq!(t.ansi[1], 0xb10b00);
    assert!(!t.dark);
    let mut l = TermLook::from_show_config(HER_SHOW, &d);
    l.put1("palette:9", "#ff0000", &d);
    l.put1("cursor-style", "bar", &d);
    l.apply_theme(&t, &d);
    assert_eq!(l.one("background", &d), "#f1ebe6");
    assert_eq!(l.one("palette:9", &d), d.get("palette:9")[0], "colors the theme leaves alone go back to default");
    assert_eq!(l.one("cursor-style", &d), "bar", "non-color options stay");
    assert!(!l.edited("theme"));
}

#[test]
fn pairs() {
    let d = d();
    let mut l = TermLook::default();
    l.put1("theme", "light:Breadog,dark:Ayu", &d);
    l.put1("font-size", "14", &d);
    assert_eq!(l.pair(), Some(("Breadog".into(), "Ayu".into())));
    let text = l.to_file("pair", &d, &pinned_keys(&[HER_CONFIG]));
    assert!(text.contains("theme = light:Breadog,dark:Ayu\n"));
    assert!(!text.contains("background ="), "a pair leaves colors to the theme: {text}");
    assert!(text.contains("adjust-cell-height = \"\""));
}

#[test]
fn include_line() {
    assert!(!has_include(HER_CONFIG.replace("config-file = ?ntex-look.ghostty\n", "").as_str()));
    assert!(has_include(HER_CONFIG));
    let plain = "font-family = JuliaMono\n";
    let with = add_include(plain, "2026-10-20");
    assert!(with.starts_with(plain) && with.ends_with("config-file = ?ntex-look.ghostty\n"));
    assert!(has_include(&with));
    assert_eq!(add_include(&with, "x"), with);
}

const CMUX: &str = "# cmux themes start\ntheme = light:GitLab Light,dark:Breadog\n# cmux themes end\nsidebar-font-size = 20\n\n# White paper\nbackground = #ffffff\nforeground = #000000\n";

#[test]
fn cmux_edits() {
    let s = sync_cmux_colors(CMUX, "#f1ebe6", "#362c24");
    assert_eq!(s, CMUX.replace("background = #ffffff", "background = #f1ebe6").replace("foreground = #000000", "foreground = #362c24"));
    assert_eq!(sync_cmux_colors(&s, "#ffffff", "#000000"), CMUX, "minimal edits, nothing else moves");
    let b = set_cmux_block(CMUX, Some("background = #111111\nfont-size = 14"));
    assert!(b.starts_with(CMUX));
    assert_eq!(cmux_block(&b).as_deref(), Some("background = #111111\nfont-size = 14\n"));
    // the block's own lines are not "synced"
    assert!(sync_cmux_colors(&b, "#eeeeee", "#000000").contains("background = #111111"));
    let b2 = set_cmux_block(&b, Some("font-size = 15"));
    assert_eq!(b2.matches(CMUX_START).count(), 1);
    assert_eq!(set_cmux_block(&b2, None), CMUX);
}

#[test]
fn cells_resolve_to_roles() {
    let d = d();
    let l = TermLook::from_show_config(HER_SHOW, &d);
    let c = Colors::of(&l, &d);
    let red = screen::Cell { ch: "M".into(), fg: Paint::Pal(1), bg: Paint::Default, flags: 0 };
    let x = screen::draw(&red, &c, false);
    assert_eq!((x.fg, x.bg), (0xaa3731, 0xffffff));
    assert_eq!(x.fg_from, Target::key("palette:1"));
    assert_eq!(x.bg_from, Target::key("background"));
    assert!(x.plain_bg);
    let sel = screen::Cell { flags: screen::SELECTED, ..red.clone() };
    let x = screen::draw(&sel, &c, false);
    assert_eq!((x.fg, x.bg), (0x000000, 0xbfdbfe));
    assert_eq!(x.bg_from, Target::key("selection-background"));
    let x = screen::draw(&red, &c, true);
    assert_eq!((x.fg, x.bg), (0xffffff, 0x007acc));
    // ntex's own colors stay what ntex says
    let fixed = screen::Cell { ch: "x".into(), fg: Paint::Rgb(0x3e3e9e), bg: Paint::Rgb(0xffffff), flags: 0 };
    assert_eq!(screen::draw(&fixed, &c, false).fg_from, Target::Fixed(0x3e3e9e));
    // bold-color = bright: bold palette 1 draws as 9
    let mut b = l.clone();
    b.put1("bold-color", "bright", &d);
    let bold = screen::Cell { flags: screen::BOLD, ..red.clone() };
    assert_eq!(screen::draw(&bold, &Colors::of(&b, &d), false).fg_from, Target::key("palette:9"));
    // minimum contrast: pale yellow on white turns black
    let mut m = l.clone();
    m.put1("palette:3", "#fff0a0", &d);
    m.put1("minimum-contrast", "3", &d);
    let y = screen::Cell { fg: Paint::Pal(3), ..red };
    assert_eq!(screen::draw(&y, &Colors::of(&m, &d), false).fg, 0x000000);
}

#[test]
fn shell_uses_all_sixteen() {
    let s = screen::shell(80);
    assert!(s.cells.iter().all(|r| r.len() == 80));
    let mut seen = [false; 16];
    for r in &s.cells {
        for c in r {
            for p in [c.fg, c.bg] {
                if let Paint::Pal(n) = p {
                    seen[n as usize] = true;
                }
            }
        }
    }
    assert!(seen.iter().all(|x| *x), "{seen:?}");
    assert_eq!(s.cursor.map(|c| c.0), Some(s.rows - 1));
}

#[test]
fn capture_json() {
    let s = screen::from_capture(r#"{"cols":3,"rows":2,"cursor":[1,2],"cells":[[["a",255,-1,1],["",-1,-1,0],["b",-1,16777215,2]],[["c",-1,-1,0]]]}"#).unwrap();
    assert_eq!((s.cols, s.rows, s.cursor), (3, 2, Some((1, 2))));
    assert_eq!(s.cells[0][0].fg, Paint::Rgb(255));
    assert_eq!(s.cells[0][0].bg, Paint::Default);
    assert_eq!(s.cells[0][2].bg, Paint::Rgb(0xffffff));
    assert_eq!(s.cells[1].len(), 3);
    assert_eq!(screen::from_capture(&screen::to_capture(&s)).unwrap(), s);
}

#[test]
fn state_json() {
    let d = d();
    let mut st = TermState::default();
    st.look.put1("font-size", "14", &d);
    st.applies = AppliesTo::GhosttyOnly;
    let j = serde_json::to_string(&st).unwrap();
    assert!(j.contains("\"ghostty-only\"") && j.contains("\"font-size\":[\"14\"]"), "{j}");
    assert_eq!(serde_json::from_str::<TermState>(&j).unwrap(), st);
}
