//! core::blocks and core::colour: the stanza, the file, the snippet, the
//! checks, and real tectonic compiles (in a temp dir holding copies of
//! ntex-notes.sty and the generated ntex-blocks.sty; the PDF is never opened).
use ntex_studio_core::model::*;
use ntex_studio_core::{blocks, codegen, colour};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn fun() -> BlockDraft {
    BlockDraft {
        title: "Fun fact".into(),
        env: "funfact".into(),
        base: BlockBase::Remark,
        numbering: Numbering::None,
        body: BodyFont::Upright,
        colour: BlockColour::Hue(145.0),
        cref: (String::new(), String::new()),
        trigger: "fun".into(),
        with_title: true,
    }
}

fn claim() -> BlockDraft {
    BlockDraft {
        title: "Claim".into(),
        env: "claim".into(),
        base: BlockBase::Theorem,
        numbering: Numbering::WithTheorems,
        body: BodyFont::Italic,
        colour: BlockColour::Palette("Blue".into()),
        // the default: the title in lower case, + s (written out in the stanza)
        cref: (String::new(), String::new()),
        trigger: String::new(),
        with_title: false,
    }
}

struct Temp {
    root: PathBuf,
    paths: Paths,
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A temp config whose templates/tex holds a copy of her ntex-notes.sty.
fn temp(tag: &str) -> Temp {
    let root = std::env::temp_dir().join(format!("ntex-core-blocks-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("templates/tex")).unwrap();
    let real = PathBuf::from(std::env::var("HOME").unwrap()).join(".config/texnvim/templates/tex/ntex-notes.sty");
    let notes = std::fs::read_to_string(real).unwrap();
    // as it was before the studio: without the \RequirePackage line
    let notes: String = notes.lines().filter(|l| !l.contains("{ntex-blocks}")).map(|l| format!("{l}\n")).collect();
    std::fs::write(root.join("templates/tex/ntex-notes.sty"), notes).unwrap();
    // ntex-notes.sty requires ntex-base.sty (fonts and palette, shared with
    // ntex-figure.sty): copy it along
    let base = PathBuf::from(std::env::var("HOME").unwrap()).join(".config/texnvim/templates/tex/ntex-base.sty");
    if base.exists() {
        std::fs::copy(&base, root.join("templates/tex/ntex-base.sty")).unwrap();
    }
    let paths = Paths { config: root.clone(), snippets: root.join("snippets"), templates_tex: root.join("templates/tex"), state: root.join("state") };
    Temp { root, paths }
}

#[test]
fn the_stanza() {
    let t = colour::tones_for_hue(145.0);
    let s = blocks::stanza_made(&fun(), "2026-10-07");
    let want = format!(
        "% ── block: funfact ── made in Ntex Studio 2026-10-07 ──\n\
         % studio: like=remark colour=hue:145 title=yes trigger=fun\n\
         \\definecolor{{ntexFunfactRule}}{{HTML}}{{{}}}\n\
         \\definecolor{{ntexFunfactBack}}{{HTML}}{{{}}}\n\
         \\definecolor{{ntexFunfactHead}}{{HTML}}{{{}}}\n\
         \\theoremstyle{{ntexupright}}\n\
         \\newtheorem*{{funfact}}{{Fun fact}}\n\
         \\tcolorboxenvironment{{funfact}}{{ntexbox=Funfact}}\n\
         % ── end: funfact ──\n",
        colour::hex(t.rule),
        colour::hex(t.back),
        colour::hex(t.head)
    );
    assert_eq!(s, want);
    // hex: six uppercase digits
    assert!(s.lines().filter(|l| l.starts_with("\\definecolor")).all(|l| {
        let h = &l[l.len() - 7..l.len() - 1];
        h.len() == 6 && h.chars().all(|c| c.is_ascii_digit() || c.is_ascii_uppercase())
    }));

    // palette colours reuse the theme's; numbered with theorems; cref names
    let s = blocks::stanza_made(&claim(), "2026-10-07");
    assert_eq!(
        s,
        "% ── block: claim ── made in Ntex Studio 2026-10-07 ──\n\
         % studio: like=theorem colour=Blue title=no\n\
         \\theoremstyle{ntexplain}\n\
         \\newtheorem{claim}[theorem]{Claim}\n\
         \\tcolorboxenvironment{claim}{ntexbox=Blue}\n\
         \\crefname{claim}{Claim}{Claims}\n\
         \\Crefname{claim}{Claim}{Claims}\n\
         % ── end: claim ──\n"
    );
    // own counter, per section; default cref names from the title
    let own = BlockDraft { numbering: Numbering::Own, cref: (String::new(), String::new()), ..fun() };
    let s = blocks::stanza_made(&own, "2026-10-07");
    assert!(s.contains("\\newtheorem{funfact}{Fun fact}[section]\n"));
    assert!(s.contains("\\crefname{funfact}{Fun fact}{Fun facts}\n\\Crefname{funfact}{Fun fact}{Fun facts}\n"));
    // the proof grey: the quiet style, no colours of its own
    let grey = BlockDraft { colour: BlockColour::Palette("Proof".into()), ..fun() };
    let s = blocks::stanza_made(&grey, "2026-10-07");
    assert!(!s.contains("\\definecolor"));
    assert!(s.contains("\\tcbset{ntexquiet/.style={"));
    assert!(s.contains("\\tcolorboxenvironment{funfact}{ntexquiet}\n"));
    // stanza() is dated today
    assert!(blocks::stanza(&fun()).starts_with(&format!("% ── block: funfact ── made in Ntex Studio {} ──\n", blocks::today())));
    // titles are escaped
    let odd = BlockDraft { title: "50% & co".into(), ..fun() };
    assert!(blocks::stanza_made(&odd, "x").contains("\\newtheorem*{funfact}{50\\% \\& co}"));
}

#[test]
fn the_file() {
    let t = temp("file");
    let p = &t.paths;
    let e = blocks::empty_file();
    assert!(e.contains("\\ProvidesPackage{ntex-blocks}"));
    assert!(e.trim_end().ends_with("\\endinput"));
    assert!(blocks::parse_file(&e).is_empty());
    assert!(blocks::read(p).unwrap().is_empty());
    blocks::remove(p, "funfact").unwrap();
    assert!(!p.blocks_sty().exists());

    blocks::save(p, &fun()).unwrap();
    let notes = std::fs::read_to_string(p.notes_sty()).unwrap();
    assert_eq!(notes.matches(blocks::REQUIRE).count(), 1);
    // just before \endinput
    let at = notes.find(blocks::REQUIRE).unwrap();
    assert!(notes[at..].trim_end().ends_with("\\endinput"));
    assert!(notes[at..].lines().filter(|l| !l.trim().is_empty()).count() == 2, "{}", &notes[at..]);

    blocks::save(p, &claim()).unwrap();
    // saving again never adds a second \RequirePackage
    assert_eq!(std::fs::read_to_string(p.notes_sty()).unwrap(), notes);
    let got = blocks::read(p).unwrap();
    assert_eq!(got.iter().map(|b| b.draft.clone()).collect::<Vec<_>>(), [fun(), claim()]);
    let file = std::fs::read_to_string(p.blocks_sty()).unwrap();
    assert!(file.starts_with(&blocks::empty_file().replace("\n\\endinput\n", "")));
    assert!(file.ends_with("% ── end: claim ──\n\n\\endinput\n"), "{file}");
    assert!(file.contains(&got[0].source));

    // replace in place (same env), keeping the date it was made
    let file = file.replace(&format!("made in Ntex Studio {} ──", blocks::today()), "made in Ntex Studio 2026-01-02 ──");
    std::fs::write(p.blocks_sty(), &file).unwrap();
    let fun2 = BlockDraft { title: "Fun facts".into(), colour: BlockColour::Palette("Teal".into()), ..fun() };
    blocks::save(p, &fun2).unwrap();
    let got = blocks::read(p).unwrap();
    assert_eq!(got.iter().map(|b| b.draft.clone()).collect::<Vec<_>>(), [fun2.clone(), claim()]);
    assert!(got[0].source.starts_with("% ── block: funfact ── made in Ntex Studio 2026-01-02 ──\n"));

    // remove; the rest stays as it was
    blocks::remove(p, "funfact").unwrap();
    let file = std::fs::read_to_string(p.blocks_sty()).unwrap();
    assert_eq!(blocks::read(p).unwrap().len(), 1);
    assert!(!file.contains("funfact"));
    blocks::remove(p, "claim").unwrap();
    assert_eq!(std::fs::read_to_string(p.blocks_sty()).unwrap(), blocks::empty_file());
    // no temp files left behind
    let names: Vec<String> = std::fs::read_dir(&p.templates_tex).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n != "ntex-base.sty").collect();
    assert_eq!(names.len(), 2, "{names:?}");
}

#[test]
fn require_line() {
    let notes = "\\ProvidesPackage{ntex-notes}\n\\tcbset{}\n\\endinput\n";
    let n = blocks::with_require(notes);
    assert_eq!(n, "\\ProvidesPackage{ntex-notes}\n\\tcbset{}\n\\RequirePackage{ntex-blocks}   % blocks made in Ntex Studio (ntex-blocks.sty)\n\n\\endinput\n");
    assert_eq!(blocks::with_require(&n), n);
    // BRIDGE's own line (any spacing/comment) counts; a commented-out one doesn't
    let bridge = "x\n\\RequirePackage{ntex-blocks} % custom blocks live in ntex-blocks.sty\n\\endinput\n";
    assert_eq!(blocks::with_require(bridge), bridge);
    let off = "x\n% \\RequirePackage{ntex-blocks}\n\\endinput\n";
    assert_eq!(blocks::with_require(off).matches("\n\\RequirePackage{ntex-blocks}").count(), 1);
    // no \endinput: at the end
    assert!(blocks::with_require("x").ends_with("x\n\\RequirePackage{ntex-blocks}   % blocks made in Ntex Studio (ntex-blocks.sty)\n"));
}

/// A file that was hand-edited: stanzas without the studio line still read.
#[test]
fn hand_written_stanzas() {
    let src = "% ── block: aside ── made in Ntex Studio 2026-10-07 ──\n\\theoremstyle{ntexupright}\n\\newtheorem*{aside}{Aside}\n\\tcolorboxenvironment{aside}{ntexbox=Maroon}\n% ── end: aside ──\n\n% ── block: note ──\n\\definecolor{ntexNoteRule}{HTML}{8A3FA0}\n\\theoremstyle{ntexplain}\n\\newtheorem{note}{Note}[section]\n\\tcolorboxenvironment{note}{ntexbox=Note}\n\\crefname{note}{remark}{remarks}\n% ── end: note ──\n\\endinput\n";
    let got = blocks::parse_file(src);
    assert_eq!(got.len(), 2);
    let a = &got[0].draft;
    assert_eq!((a.title.as_str(), a.env.as_str(), a.base, a.numbering, a.body), ("Aside", "aside", BlockBase::Example, Numbering::None, BodyFont::Upright));
    assert_eq!(a.colour, BlockColour::Palette("Maroon".into()));
    let n = &got[1].draft;
    assert_eq!((n.numbering, n.body, n.base), (Numbering::Own, BodyFont::Italic, BlockBase::Theorem));
    assert_eq!(n.colour, BlockColour::Hue(colour::hue_of(Rgb(0x8A, 0x3F, 0xA0)).round()));
    assert_eq!(n.cref, ("remark".to_string(), "remarks".to_string()));
}

#[test]
fn the_snippet() {
    assert!(blocks::snippet(&BlockDraft { trigger: " ".into(), ..fun() }).is_none());
    let d = blocks::snippet(&fun()).unwrap();
    assert_eq!((d.kind, d.context, d.fires, d.line, d.ft), (Kind::Plain, Context::Text, Fires::Tab, LineRule::Start, FileType::Tex));
    assert_eq!((d.origin.as_str(), d.desc.as_str(), d.trigger.as_str()), ("yours", "Fun fact block", "fun"));
    assert_eq!(codegen::body(&d.output), "\\begin{funfact}[<>]\n\t<>\n\\end{funfact}");
    let e = codegen::entry(&d);
    assert!(e.contains("text(\"fun\", [[\n\\begin{funfact}[<>]\n\t<>\n\\end{funfact}]], \"yours\", { tab = true, line = \"start\", ft = \"tex\", desc = \"Fun fact block\" }),"), "{e}");
    // and it reads back as the same draft
    let back = ntex_studio_core::luaparse::parse_entry(&e).unwrap();
    assert_eq!(codegen::body(&back.output), codegen::body(&d.output));
    let d = blocks::snippet(&BlockDraft { with_title: false, ..fun() }).unwrap();
    assert_eq!(codegen::body(&d.output), "\\begin{funfact}\n\t<>\n\\end{funfact}");
}

#[test]
fn the_checks() {
    let codes = |b: &BlockDraft, existing: &[KnownBlock]| -> Vec<String> { blocks::check(b, existing, &[]).into_iter().map(|f| f.code).collect() };
    let none = BlockDraft { trigger: String::new(), ..fun() };
    assert!(codes(&none, &[]).is_empty(), "{:?}", blocks::check(&none, &[], &[]));
    assert_eq!(codes(&BlockDraft { title: " ".into(), ..none.clone() }, &[]), ["empty-title"]);
    assert_eq!(codes(&BlockDraft { env: "".into(), ..none.clone() }, &[]), ["empty-env"]);
    let bad = blocks::check(&BlockDraft { env: "fun_fact2".into(), ..none.clone() }, &[], &[]);
    assert_eq!(bad[0].code, "env-letters");
    assert_eq!(bad[0].severity, Severity::Error);
    assert_eq!(bad[0].fix.as_ref().unwrap().block.as_ref().unwrap().env, "funfact");
    for taken in ["theorem", "lemma", "proposition", "corollary", "definition", "example", "remark", "proof", "align", "equation", "itemize", "tcolorbox", "box", "blue"] {
        assert_eq!(codes(&BlockDraft { env: taken.into(), ..none.clone() }, &[]), ["env-taken"], "{taken}");
    }
    let existing = vec![KnownBlock { draft: fun(), source: String::new() }];
    assert_eq!(codes(&none, &existing), ["env-exists"]);
    assert_eq!(codes(&BlockDraft { colour: BlockColour::Palette("Pink".into()), ..none.clone() }, &[]), ["unknown-colour"]);
    // every palette colour and every hue passes the contrast checks
    for (name, _) in colour::palette() {
        assert!(codes(&BlockDraft { colour: BlockColour::Palette(name.into()), ..none.clone() }, &[]).is_empty(), "{name}");
    }
    for h in (0..360).step_by(3) {
        assert!(codes(&BlockDraft { colour: BlockColour::Hue(h as f32), ..none.clone() }, &[]).is_empty(), "{h}");
    }
    // with a trigger, the snippet's own checks run (a duplicate of a loaded Tab snippet)
    let known = vec![KnownSnippet {
        trigger: "fun".into(),
        pattern: None,
        kind: "text".into(),
        where_: "text".into(),
        how: "tab".into(),
        body: Some("x".into()),
        file: "text.lua".into(),
        line: Some(3),
        origin: "yours".into(),
        group: None,
        line_rule: Some("start".into()),
        ft: Some("tex".into()),
        priority: 0,
        desc: None,
    }];
    let f = blocks::check(&fun(), &[], &known);
    assert!(!f.is_empty(), "a conflicting trigger should be reported");
    assert!(f.iter().all(|x| x.severity != Severity::Note));
    eprintln!("trigger findings: {:?}", f.iter().map(|x| (&x.code, &x.message)).collect::<Vec<_>>());
}

#[test]
fn env_names() {
    assert_eq!(blocks::env_from_title("Fun fact"), "funfact");
    assert_eq!(blocks::env_from_title("Côté"), "cote");
    assert_eq!(blocks::env_from_title("Naïve Bayes’ rule (2)"), "naivebayesrule");
    assert_eq!(blocks::env_from_title("Γ-thing"), "thing");
}

#[test]
fn the_specimen() {
    let s = blocks::specimen(&fun());
    assert!(s.contains("\\documentclass[11pt]{article}\n\\usepackage{ntex-notes}\n"));
    assert!(s.contains("\\begin{remark}\n"));
    assert!(s.contains("\\newtheorem*{ntexspecimen}{Fun fact}"));
    assert!(s.contains("\\begin{ntexspecimen}[Specimen]\n"));
    let s = blocks::specimen(&claim());
    assert!(s.contains("\\begin{theorem}\n") && s.contains("\\begin{ntexspecimen}\n"));
}

// ---------------------------------------------------------------------------
// real compiles

fn tectonic() -> Option<PathBuf> {
    ["/opt/homebrew/bin/tectonic", "/usr/local/bin/tectonic"].iter().map(PathBuf::from).find(|p| p.exists())
}

/// `tectonic -Z search-path=<dir> <name>` in `dir`, killed after 5 minutes.
fn compile(dir: &Path, name: &str) -> Result<(), String> {
    let mut child = Command::new(tectonic().unwrap())
        .arg("-Z")
        .arg(format!("search-path={}", dir.display()))
        .args(["--keep-intermediates", "--keep-logs", "--chatter", "minimal", name])
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let start = Instant::now();
    while child.try_wait().map_err(|e| e.to_string())?.is_none() {
        if start.elapsed() > Duration::from_secs(300) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("tectonic timed out".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!("{}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
    }
}

#[test]
fn compiles_with_tectonic() {
    if tectonic().is_none() {
        eprintln!("tectonic not installed: skipped");
        return;
    }
    let t = temp("tectonic");
    let p = &t.paths;
    let remarkish = BlockDraft {
        title: "Aside".into(),
        env: "aside".into(),
        base: BlockBase::Remark,
        numbering: Numbering::None,
        body: BodyFont::Upright,
        colour: BlockColour::Palette("Green".into()),
        cref: (String::new(), String::new()),
        trigger: String::new(),
        with_title: true,
    };
    let hue = BlockDraft { title: "Fun fact".into(), numbering: Numbering::Own, colour: BlockColour::Hue(300.0), ..fun() };
    let grey = BlockDraft { title: "Quiet note".into(), env: "quietnote".into(), colour: BlockColour::Palette("Proof".into()), ..remarkish.clone() };
    for b in [&remarkish, &claim(), &hue, &grey] {
        assert!(blocks::check(b, &[], &[]).iter().all(|f| f.severity != Severity::Error), "{}", b.env);
        blocks::save(p, b).unwrap();
    }
    let dir = &p.templates_tex;
    let notes = std::fs::read_to_string(p.notes_sty()).unwrap();
    assert_eq!(notes.matches(blocks::REQUIRE).count(), 1);

    // each block's specimen (it defines its own copy, so it compiles even
    // though the same block is in ntex-blocks.sty)
    for (i, b) in [&remarkish, &claim(), &hue, &grey].into_iter().enumerate() {
        let name = format!("spec{i}.tex");
        std::fs::write(dir.join(&name), blocks::specimen(b)).unwrap();
        compile(dir, &name).unwrap_or_else(|e| panic!("specimen of {}: {e}", b.env));
        assert!(dir.join(format!("spec{i}.pdf")).exists());
    }

    // the blocks themselves, from ntex-blocks.sty, with \label and \cref
    let doc = r"\documentclass{article}
\usepackage{ntex-notes}
\begin{document}
\section{First}
\begin{theorem}\label{thm:a} Every bounded monotone sequence converges. \end{theorem}
\begin{claim}[Named]\label{cl:b} The claim holds: $x^{2} \geq 0$. \end{claim}
\begin{funfact}\label{ff:c} A fun fact: $e^{i\pi} + 1 = 0$. \end{funfact}
\begin{aside}[Title] An aside, $\sqrt{2} \notin \mathbb{Q}$. \end{aside}
\begin{quietnote} A quiet note, $1 + 1 = 2$. \end{quietnote}
See \cref{cl:b}, \cref{ff:c} and \cref{thm:a}.
\end{document}
";
    std::fs::write(dir.join("use.tex"), doc).unwrap();
    compile(dir, "use.tex").unwrap_or_else(|e| panic!("use.tex: {e}"));
    let aux = std::fs::read_to_string(dir.join("use.aux")).unwrap();
    // claim shares the theorem counter (1.2) and is referenced as a claim;
    // funfact has its own counter per section (1.1)
    assert!(aux.contains("\\newlabel{cl:b@cref}{{[claim][2][1]1.2}"), "{aux}");
    assert!(aux.contains("\\newlabel{ff:c@cref}{{[funfact][1][1]1.1}"), "{aux}");
    assert!(aux.contains("\\newlabel{thm:a@cref}{{[theorem][1][1]1.1}"), "{aux}");
    let log = std::fs::read_to_string(dir.join("use.log")).unwrap();
    assert!(!log.contains("undefined"), "undefined references:\n{log}");
    assert!(!log.contains("cref reference format for label type"), "{log}");
    // NTEX_SPECIMEN_OUT=<dir>: keep the PDFs (to rasterise and look at; never opened here)
    if let Some(out) = std::env::var_os("NTEX_SPECIMEN_OUT") {
        let out = PathBuf::from(out);
        std::fs::create_dir_all(&out).unwrap();
        for n in ["spec0.pdf", "spec1.pdf", "spec2.pdf", "spec3.pdf", "use.pdf"] {
            std::fs::copy(dir.join(n), out.join(n)).unwrap();
        }
    }
    eprintln!("tectonic: 4 specimens + use.tex compiled; cref types ok");
}
