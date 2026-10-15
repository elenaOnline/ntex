//! Unicode → LaTeX (input) and LaTeX → Unicode (the canvas's quick preview).
//!
//! One fixed table, used both ways. Greek takes her var-forms (φ → `\varphi`,
//! ε → `\varepsilon`; ϕ and ϵ give the plain forms), ∅ is `\varnothing` (her
//! notes' choice), and runs of sub/superscript characters become one group
//! (x₁₂ → `x_{12}`, g⁻¹ → `g^{-1}`).

/// (character, LaTeX) — the input table. Order matters only for `preview`'s
/// reverse lookup: the first entry for a command wins.
const TABLE: &[(char, &str)] = &[
    // Greek, lower case (her var-forms first)
    ('α', r"\alpha"), ('β', r"\beta"), ('γ', r"\gamma"), ('δ', r"\delta"),
    ('ε', r"\varepsilon"), ('ϵ', r"\epsilon"), ('ζ', r"\zeta"), ('η', r"\eta"),
    ('θ', r"\theta"), ('ϑ', r"\vartheta"), ('ι', r"\iota"), ('κ', r"\kappa"),
    ('λ', r"\lambda"), ('μ', r"\mu"), ('ν', r"\nu"), ('ξ', r"\xi"), ('ο', "o"),
    ('π', r"\pi"), ('ϖ', r"\varpi"), ('ρ', r"\rho"), ('ϱ', r"\varrho"),
    ('σ', r"\sigma"), ('ς', r"\varsigma"), ('τ', r"\tau"), ('υ', r"\upsilon"),
    ('φ', r"\varphi"), ('ϕ', r"\phi"), ('χ', r"\chi"), ('ψ', r"\psi"), ('ω', r"\omega"),
    // Greek, upper case
    ('Α', "A"), ('Β', "B"), ('Γ', r"\Gamma"), ('Δ', r"\Delta"), ('Ε', "E"), ('Ζ', "Z"),
    ('Η', "H"), ('Θ', r"\Theta"), ('Ι', "I"), ('Κ', "K"), ('Λ', r"\Lambda"), ('Μ', "M"),
    ('Ν', "N"), ('Ξ', r"\Xi"), ('Ο', "O"), ('Π', r"\Pi"), ('Ρ', "P"), ('Σ', r"\Sigma"),
    ('Τ', "T"), ('Υ', r"\Upsilon"), ('Φ', r"\Phi"), ('Χ', "X"), ('Ψ', r"\Psi"), ('Ω', r"\Omega"),
    // double-struck
    ('ℤ', r"\mathbb{Z}"), ('ℚ', r"\mathbb{Q}"), ('ℝ', r"\mathbb{R}"), ('ℂ', r"\mathbb{C}"),
    ('ℕ', r"\mathbb{N}"), ('ℙ', r"\mathbb{P}"), ('ℍ', r"\mathbb{H}"), ('𝔽', r"\mathbb{F}"),
    ('𝕜', r"\Bbbk"),
    // operators and relations
    ('∘', r"\circ"), ('×', r"\times"), ('⊗', r"\otimes"), ('⊕', r"\oplus"), ('≅', r"\cong"),
    ('⊂', r"\subset"), ('⊆', r"\subseteq"), ('⊊', r"\subsetneq"), ('⊃', r"\supset"),
    ('⊇', r"\supseteq"), ('∈', r"\in"), ('∉', r"\notin"), ('∋', r"\ni"), ('∩', r"\cap"),
    ('∪', r"\cup"), ('∅', r"\varnothing"), ('∞', r"\infty"), ('≤', r"\leq"), ('≥', r"\geq"),
    ('≠', r"\neq"), ('↦', r"\mapsto"), ('→', r"\to"), ('←', r"\leftarrow"),
    ('↪', r"\hookrightarrow"), ('↠', r"\twoheadrightarrow"), ('⇒', r"\Rightarrow"),
    ('⇔', r"\Leftrightarrow"), ('⟹', r"\implies"), ('⟺', r"\iff"), ('↔', r"\leftrightarrow"),
    ('≃', r"\simeq"), ('≡', r"\equiv"), ('∼', r"\sim"), ('≈', r"\approx"), ('⊲', r"\lhd"),
    ('◁', r"\triangleleft"), ('⊴', r"\trianglelefteq"), ('⋊', r"\rtimes"), ('⋉', r"\ltimes"),
    ('·', r"\cdot"), ('⋅', r"\cdot"), ('∀', r"\forall"), ('∃', r"\exists"), ('¬', r"\neg"),
    ('∧', r"\wedge"), ('∨', r"\vee"), ('∂', r"\partial"), ('∇', r"\nabla"), ('∑', r"\sum"),
    ('∏', r"\prod"), ('∐', r"\coprod"), ('⟨', r"\langle"), ('⟩', r"\rangle"), ('±', r"\pm"),
    ('∓', r"\mp"), ('∖', r"\setminus"), ('⊥', r"\perp"), ('ℓ', r"\ell"), ('⋆', r"\star"),
    ('∣', r"\mid"), ('∤', r"\nmid"), ('⊔', r"\sqcup"), ('⨿', r"\amalg"), ('…', r"\dots"),
    ('⋯', r"\cdots"), ('√', r"\surd"), ('†', r"\dagger"), ('⊢', r"\vdash"), ('∫', r"\int"),
    ('⊞', r"\boxplus"), ('≪', r"\ll"), ('≫', r"\gg"), ('∝', r"\propto"), ('⊏', r"\sqsubset"),
    ('ℵ', r"\aleph"), ('∗', "*"), ('−', "-"), ('′', "'"), ('″', "''"),
];

const SUPERS: &[(char, char)] = &[
    ('⁰', '0'), ('¹', '1'), ('²', '2'), ('³', '3'), ('⁴', '4'), ('⁵', '5'), ('⁶', '6'),
    ('⁷', '7'), ('⁸', '8'), ('⁹', '9'), ('⁺', '+'), ('⁻', '-'), ('⁼', '='), ('⁽', '('),
    ('⁾', ')'), ('ⁿ', 'n'), ('ⁱ', 'i'), ('ᵃ', 'a'), ('ᵇ', 'b'), ('ᶜ', 'c'), ('ᵈ', 'd'),
    ('ᵉ', 'e'), ('ᶠ', 'f'), ('ᵍ', 'g'), ('ʰ', 'h'), ('ʲ', 'j'), ('ᵏ', 'k'), ('ˡ', 'l'),
    ('ᵐ', 'm'), ('ᵒ', 'o'), ('ᵖ', 'p'), ('ʳ', 'r'), ('ˢ', 's'), ('ᵗ', 't'), ('ᵘ', 'u'),
    ('ᵛ', 'v'), ('ʷ', 'w'), ('ˣ', 'x'), ('ʸ', 'y'), ('ᶻ', 'z'), ('ᴬ', 'A'), ('ᴮ', 'B'),
    ('ᴰ', 'D'), ('ᴱ', 'E'), ('ᴳ', 'G'), ('ᴴ', 'H'), ('ᴵ', 'I'), ('ᴶ', 'J'), ('ᴷ', 'K'),
    ('ᴸ', 'L'), ('ᴹ', 'M'), ('ᴺ', 'N'), ('ᴼ', 'O'), ('ᴾ', 'P'), ('ᴿ', 'R'), ('ᵀ', 'T'),
    ('ᵁ', 'U'), ('ⱽ', 'V'), ('ᵂ', 'W'), ('′', '\''),
];

const SUBS: &[(char, char)] = &[
    ('₀', '0'), ('₁', '1'), ('₂', '2'), ('₃', '3'), ('₄', '4'), ('₅', '5'), ('₆', '6'),
    ('₇', '7'), ('₈', '8'), ('₉', '9'), ('₊', '+'), ('₋', '-'), ('₌', '='), ('₍', '('),
    ('₎', ')'), ('ₐ', 'a'), ('ₑ', 'e'), ('ₒ', 'o'), ('ₓ', 'x'), ('ₕ', 'h'), ('ₖ', 'k'),
    ('ₗ', 'l'), ('ₘ', 'm'), ('ₙ', 'n'), ('ₚ', 'p'), ('ₛ', 's'), ('ₜ', 't'), ('ᵢ', 'i'),
    ('ⱼ', 'j'), ('ᵣ', 'r'), ('ᵤ', 'u'), ('ᵥ', 'v'),
];

fn lookup(c: char) -> Option<&'static str> {
    TABLE.iter().find(|(k, _)| *k == c).map(|(_, v)| *v)
}
fn sup_of(c: char) -> Option<char> {
    // ′ is a prime, not a superscript run: handled by TABLE.
    if c == '′' {
        return None;
    }
    SUPERS.iter().find(|(k, _)| *k == c).map(|(_, v)| *v)
}
fn sub_of(c: char) -> Option<char> {
    SUBS.iter().find(|(k, _)| *k == c).map(|(_, v)| *v)
}

/// "φ(G) ≅ ℤ/nℤ" → "\varphi(G) \cong \mathbb{Z}/n\mathbb{Z}". Fixed table (FIGURES.md).
/// ASCII passes through untouched, so this is idempotent on LaTeX.
pub fn to_latex(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len() + 8);
    // a control word was just written: a following ASCII letter needs a space
    let mut pending_space = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if sup_of(c).is_some() || sub_of(c).is_some() {
            let sup = sup_of(c).is_some();
            let mut run = String::new();
            while i < chars.len() {
                let m = if sup { sup_of(chars[i]) } else { sub_of(chars[i]) };
                match m {
                    Some(a) => run.push(a),
                    None => break,
                }
                i += 1;
            }
            out.push(if sup { '^' } else { '_' });
            out.push('{');
            out.push_str(&run);
            out.push('}');
            pending_space = false;
            continue;
        }
        if let Some(rep) = lookup(c) {
            if pending_space && rep.starts_with(|ch: char| ch.is_ascii_alphabetic()) {
                out.push(' ');
            }
            out.push_str(rep);
            pending_space = rep.starts_with('\\') && rep.ends_with(|ch: char| ch.is_ascii_alphabetic());
        } else {
            if pending_space && c.is_ascii_alphabetic() {
                out.push(' ');
            }
            out.push(c);
            pending_space = false;
        }
        i += 1;
    }
    out
}

/// For LaTeX *text* (free `text` items): symbols become `$…$` maths islands,
/// letters (Greek, ℤ) too, so "kernel ↦ identity" → "kernel $\mapsto$ identity".
pub fn to_latex_text(s: &str) -> String {
    let mut out = String::new();
    let mut math = String::new();
    let flush = |out: &mut String, math: &mut String| {
        if !math.is_empty() {
            out.push('$');
            out.push_str(&to_latex(math));
            out.push('$');
            math.clear();
        }
    };
    for c in s.chars() {
        if c.is_ascii() || (lookup(c).is_none() && sup_of(c).is_none() && sub_of(c).is_none()) {
            flush(&mut out, &mut math);
            out.push(c);
        } else {
            math.push(c);
        }
    }
    flush(&mut out, &mut math);
    out
}

// ---------------------------------------------------------------------------
// preview: LaTeX → readable Unicode

/// Operator names that preview as upright words (followed by a space before a letter).
const WORDS: &[&str] = &[
    "ker", "im", "Im", "Re", "Hom", "End", "Aut", "Inn", "Out", "Gal", "id", "coker", "sin",
    "cos", "tan", "log", "ln", "exp", "det", "dim", "deg", "gcd", "lcm", "max", "min", "sup",
    "inf", "lim", "liminf", "limsup", "mod", "bmod", "tr", "rank", "sgn", "Sym", "GL", "SL",
    "PGL", "PSL", "Spec", "char", "ord", "Stab", "Orb", "Fix", "arg", "Pr",
];

/// Commands not in TABLE (or with a better reverse).
const EXTRA: &[(&str, &str)] = &[
    ("phi", "ϕ"), ("epsilon", "ϵ"), ("emptyset", "∅"), ("le", "≤"), ("ge", "≥"),
    ("ne", "≠"), ("rightarrow", "→"), ("longrightarrow", "⟶"), ("mapsto", "↦"),
    ("longmapsto", "⟼"), ("hookrightarrow", "↪"), ("twoheadrightarrow", "↠"),
    ("Leftarrow", "⇐"), ("gets", "←"), ("normalsubgroup", "⊲"), ("unlhd", "⊴"),
    ("trianglelefteq", "⊴"), ("lhd", "⊲"), ("triangleleft", "◁"), ("ldots", "…"),
    ("vdots", "⋮"), ("ddots", "⋱"), ("colon", ":"), ("lbrace", "{"), ("rbrace", "}"),
    ("langle", "⟨"), ("rangle", "⟩"), ("lvert", "|"), ("rvert", "|"), ("vert", "|"),
    ("Vert", "‖"), ("mid", "|"), ("backslash", "\\"), ("bigoplus", "⊕"), ("bigotimes", "⊗"),
    ("bigcup", "⋃"), ("bigcap", "⋂"), ("bigsqcup", "⊔"), ("land", "∧"), ("lor", "∨"),
    ("lnot", "¬"), ("iso", "≅"), ("sqrt", "√"), ("quad", "  "), ("qquad", "   "),
    ("cdot", "·"), ("times", "×"), ("prime", "′"), ("ast", "∗"), ("dagger", "†"),
    ("lfloor", "⌊"), ("rfloor", "⌋"), ("lceil", "⌈"), ("rceil", "⌉"), ("divides", "∣"),
    ("nsubseteq", "⊈"), ("supsetneq", "⊋"), ("simeq", "≃"), ("models", "⊨"),
    ("Bbbk", "𝕜"), ("hbar", "ℏ"), ("wp", "℘"), ("Re", "ℜ"),
];

/// Binary operators and relations: previewed with a space on each side.
const SPACED: &[&str] = &[
    "circ", "times", "otimes", "oplus", "cong", "subset", "subseteq", "subsetneq", "supset",
    "supseteq", "in", "notin", "ni", "cap", "cup", "leq", "geq", "neq", "le", "ge", "ne",
    "mapsto", "longmapsto", "to", "rightarrow", "longrightarrow", "leftarrow", "gets",
    "hookrightarrow", "twoheadrightarrow", "Rightarrow", "Leftarrow", "Leftrightarrow",
    "implies", "iff", "leftrightarrow", "simeq", "equiv", "sim", "approx", "lhd",
    "triangleleft", "trianglelefteq", "unlhd", "rtimes", "ltimes", "cdot", "wedge", "vee",
    "land", "lor", "pm", "mp", "setminus", "mid", "nmid", "sqcup", "amalg", "vdash", "models",
    "boxplus", "ll", "gg", "propto", "iso", "divides", "nsubseteq", "supsetneq",
];

/// Commands that take one argument and keep its content as is.
const STYLE: &[&str] = &[
    "mathrm", "text", "textrm", "textit", "textbf", "mathbf", "mathit", "mathsf", "mathtt",
    "mathcal", "mathscr", "mathfrak", "boldsymbol", "bm", "mbox", "textsf", "texttt", "emph",
    "mathnormal", "displaystyle", "textstyle", "scriptstyle",
];

/// Commands that vanish (sizing / spacing).
const DROP: &[&str] = &[
    "left", "right", "big", "Big", "bigg", "Bigg", "bigl", "bigr", "Bigl", "Bigr", "biggl",
    "biggr", "middle", "limits", "nolimits", "displaystyle", "textstyle", "scriptstyle",
    "nonumber", "relax", "mathstrut", "strut",
];

/// "\varphi(G)_{1}" → "φ(G)₁": a readable plain-text approximation for the
/// canvas while dragging (Greek, blackboard, common symbols, simple
/// sub/superscripts; anything else stays as written without the backslash).
pub fn preview(latex: &str) -> String {
    preview_runs(latex).into_iter().map(|(text, _)| text).collect()
}

/// Draft maths preview, retaining text/math boundaries. The bool selects
/// the serif italic face; digits, operators and text are upright.
pub fn preview_runs(latex: &str) -> Vec<(String, bool)> {
    styled_preview(latex, Mode::MATH)
}

/// Free text labels start upright, with italic maths letters in `$…$` islands.
pub fn preview_text_runs(latex: &str) -> Vec<(String, bool)> {
    styled_preview(latex, Mode::TEXT)
}

#[derive(Clone, Copy)]
struct Mode {
    text: bool,
    italic: bool,
}
impl Mode {
    const MATH: Self = Self { text: false, italic: true };
    const TEXT: Self = Self { text: true, italic: false };
    const ROMAN: Self = Self { text: false, italic: false };
}

type StyledChars = Vec<(char, bool)>;

fn styled_preview(latex: &str, mode: Mode) -> Vec<(String, bool)> {
    let chars: Vec<char> = latex.chars().collect();
    let mut p = Preview { s: &chars, i: 0 };
    let chars = join_pieces(&p.seq(None, mode));
    let mut out: Vec<(String, bool)> = Vec::new();
    let mut space = false;
    // Collapse and trim spaces without losing the style of adjacent glyphs.
    for (c, italic) in chars {
        if c == ' ' {
            space = !out.is_empty();
            continue;
        }
        if space {
            push_run(&mut out, ' ', false);
            space = false;
        }
        push_run(&mut out, c, italic);
    }
    out
}

fn push_run(out: &mut Vec<(String, bool)>, c: char, italic: bool) {
    if let Some((text, style)) = out.last_mut()
        && *style == italic
    {
        text.push(c);
    } else {
        out.push((c.to_string(), italic));
    }
}

/// Operator words want a space before a following letter or digit.
struct Piece {
    chars: StyledChars,
    word: bool,
}
impl Piece {
    fn new(text: &str, mode: Mode, word: bool) -> Self {
        Self {
            chars: text.chars().map(|c| {
                // TeX's punctuation and digits are upright. Greek capitals
                // also keep the draft's existing upright convention.
                let letter = c.is_alphabetic() && !('Α'..='Ω').contains(&c);
                (c, mode.italic && letter)
            }).collect(),
            word,
        }
    }
    fn text(&self) -> String {
        self.chars.iter().map(|(c, _)| c).collect()
    }
}

fn join_pieces(pieces: &[Piece]) -> StyledChars {
    let mut out = Vec::new();
    let mut want_space = false;
    for p in pieces {
        let Some((first, _)) = p.chars.first() else { continue };
        if want_space && first.is_alphanumeric() {
            out.push((' ', false));
        }
        out.extend_from_slice(&p.chars);
        want_space = p.word;
    }
    out
}

struct Preview<'a> {
    s: &'a [char],
    i: usize,
}

impl Preview<'_> {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }

    /// Pieces until `close` (consumed) or the end. Modes are scoped to
    /// groups, so returning from a text command restores maths italics.
    fn seq(&mut self, close: Option<char>, mode: Mode) -> Vec<Piece> {
        let mut out = Vec::new();
        while let Some(c) = self.peek() {
            if Some(c) == close {
                self.i += 1;
                break;
            }
            match c {
                '$' if mode.text => {
                    self.i += 1;
                    out.extend(self.seq(Some('$'), Mode::MATH));
                }
                '^' | '_' if !mode.text => {
                    self.i += 1;
                    let arg = self.arg(mode);
                    let text = arg.text();
                    let rendered = script(&text, c == '^');
                    let mapped: Vec<char> = rendered.chars().collect();
                    let chars = if mapped.len() == arg.chars.len() {
                        mapped.into_iter().zip(arg.chars.iter().map(|(_, style)| *style)).collect()
                    } else {
                        // Unavailable Unicode scripts retain each argument's
                        // styling in the readable ^(...) / _(...) fallback.
                        let mut chars = vec![(c, false)];
                        let parens = text.chars().count() > 1 && !text.chars().all(char::is_alphanumeric);
                        if parens { chars.push(('(', false)); }
                        chars.extend(arg.chars);
                        if parens { chars.push((')', false)); }
                        chars
                    };
                    out.push(Piece { chars, word: false });
                }
                c if c.is_whitespace() && !mode.text => self.i += 1,
                _ => {
                    if let Some(p) = self.atom(mode) { out.push(p); }
                }
            }
        }
        out
    }

    fn arg(&mut self, mode: Mode) -> Piece {
        while self.peek() == Some(' ') { self.i += 1; }
        if self.peek() == Some('{') {
            self.i += 1;
            let pieces = self.seq(Some('}'), mode);
            Piece { chars: join_pieces(&pieces), word: pieces.last().is_some_and(|p| p.word) }
        } else {
            self.atom(mode).unwrap_or_else(|| Piece::new("", mode, false))
        }
    }

    fn atom(&mut self, mode: Mode) -> Option<Piece> {
        let c = self.peek()?;
        self.i += 1;
        match c {
            '{' => {
                let ps = self.seq(Some('}'), mode);
                Some(Piece { chars: join_pieces(&ps), word: ps.last().is_some_and(|p| p.word) })
            }
            '}' | '$' => None,
            '~' => Some(Piece::new(" ", mode, false)),
            '\\' => self.command(mode),
            _ => Some(Piece::new(&c.to_string(), mode, false)),
        }
    }

    fn command(&mut self, mode: Mode) -> Option<Piece> {
        let start = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_alphabetic()) { self.i += 1; }
        if self.i == start {
            let c = self.peek()?;
            self.i += 1;
            let t = match c {
                ',' | ';' | ':' | ' ' | '>' | '\\' => " ".to_string(),
                '!' | '/' => String::new(),
                '|' => "‖".to_string(),
                _ => c.to_string(),
            };
            return Some(Piece::new(&t, mode, false));
        }
        let name: String = self.s[start..self.i].iter().collect();
        if self.peek() == Some(' ') { self.i += 1; }
        if DROP.contains(&name.as_str()) {
            if (name == "left" || name == "right") && self.peek() == Some('.') { self.i += 1; }
            return None;
        }
        if WORDS.contains(&name.as_str()) {
            return Some(Piece::new(&name, Mode::ROMAN, true));
        }
        match name.as_str() {
            "mathbb" | "mathbbm" | "bbold" => {
                let a = self.arg(mode).text();
                return Some(Piece::new(&a.chars().map(blackboard).collect::<String>(), Mode::ROMAN, false));
            }
            "operatorname" => {
                if self.peek() == Some('*') { self.i += 1; }
                let mut a = self.arg(Mode::ROMAN);
                a.word = true;
                return Some(a);
            }
            "frac" | "dfrac" | "tfrac" => {
                let wrap = |mut p: Piece| {
                    let text = p.text();
                    if text.chars().count() > 1 && text.contains(|c: char| " +-".contains(c)) {
                        p.chars.insert(0, ('(', false));
                        p.chars.push((')', false));
                    }
                    p
                };
                let mut a = wrap(self.arg(mode));
                let b = wrap(self.arg(mode));
                a.chars.push(('/', false));
                a.chars.extend(b.chars);
                a.word = false;
                return Some(a);
            }
            "sqrt" => {
                let mut a = self.arg(mode);
                a.chars.insert(0, ('√', false));
                a.word = false;
                return Some(a);
            }
            "overline" | "bar" => return Some(self.combining(mode, '\u{0305}', '\u{0304}')),
            "hat" | "widehat" => return Some(self.combining(mode, '\u{0302}', '\u{0302}')),
            "tilde" | "widetilde" => return Some(self.combining(mode, '\u{0303}', '\u{0303}')),
            "dot" => return Some(self.combining(mode, '\u{0307}', '\u{0307}')),
            "vec" => return Some(self.combining(mode, '\u{20D7}', '\u{20D7}')),
            "underline" => return Some(self.combining(mode, '\u{0332}', '\u{0332}')),
            "not" => {
                let mut a = self.arg(mode);
                let t = match a.text().as_str() { "=" => Some("≠"), "∈" => Some("∉"), "⊂" => Some("⊄"), _ => None };
                if let Some(t) = t { return Some(Piece::new(t, mode, false)); }
                a.chars.push(('\u{0338}', a.chars.last().is_some_and(|(_, italic)| *italic)));
                a.word = false;
                return Some(a);
            }
            _ => {}
        }
        if name.starts_with("text") || name == "mbox" {
            let mut a = self.arg(Mode { text: true, italic: name == "textit" });
            a.word = true;
            return Some(a);
        }
        if STYLE.contains(&name.as_str()) {
            if self.peek() == Some('{') {
                let style = match name.as_str() {
                    "mathrm" | "mathbf" | "mathsf" | "mathtt" => Mode::ROMAN,
                    "mathit" | "mathnormal" | "emph" => Mode { italic: true, ..mode },
                    _ => mode,
                };
                let mut a = self.arg(style);
                a.word = name == "mathrm";
                return Some(a);
            }
            return None;
        }
        let spaced = |t: String| if SPACED.contains(&name.as_str()) { format!(" {t} ") } else { t };
        if let Some((_, t)) = EXTRA.iter().find(|(k, _)| *k == name) {
            return Some(Piece::new(&spaced(t.to_string()), mode, false));
        }
        let cmd = format!("\\{name}");
        if let Some((c, _)) = TABLE.iter().find(|(_, v)| *v == cmd) {
            return Some(Piece::new(&spaced(c.to_string()), mode, false));
        }
        Some(Piece::new(&name, mode, true))
    }

    fn combining(&mut self, mode: Mode, multi: char, single: char) -> Piece {
        let a = self.arg(mode);
        let mark = if a.chars.len() == 1 { single } else { multi };
        Piece { chars: a.chars.into_iter().flat_map(|(c, italic)| [(c, italic), (mark, italic)]).collect(), word: false }
    }
}

fn blackboard(c: char) -> char {
    match c {
        'C' => 'ℂ', 'H' => 'ℍ', 'N' => 'ℕ', 'P' => 'ℙ', 'Q' => 'ℚ', 'R' => 'ℝ', 'Z' => 'ℤ',
        'A'..='Z' => char::from_u32(0x1D538 + (c as u32 - 'A' as u32)).unwrap_or(c),
        'k' => '𝕜',
        _ => c,
    }
}

/// `^{ab}` → "ᵃᵇ" when every character has a superscript form, else "^ab"
/// (or "^(a+b)" when the content has operators or spaces).
fn script(arg: &str, sup: bool) -> String {
    let arg = arg.trim();
    if arg.is_empty() {
        return String::new();
    }
    let mapped: Option<String> = arg
        .chars()
        .map(|c| {
            if sup {
                if c == '\'' || c == '′' {
                    return Some('′');
                }
                SUPERS.iter().find(|(_, v)| *v == c).map(|(k, _)| *k)
            } else {
                SUBS.iter().find(|(_, v)| *v == c).map(|(k, _)| *k)
            }
        })
        .collect();
    if let Some(m) = mapped {
        return m;
    }
    let mark = if sup { '^' } else { '_' };
    if arg.chars().count() == 1 || arg.chars().all(|c| c.is_alphanumeric()) {
        format!("{mark}{arg}")
    } else {
        format!("{mark}({arg})")
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn runs(parts: &[(&str, bool)]) -> Vec<(String, bool)> {
        parts.iter().map(|(s, italic)| (s.to_string(), *italic)).collect()
    }

    #[test]
    fn upright_text_and_roman_scopes_restore_math_italics() {
        for command in ["text", "mathrm", "operatorname"] {
            let label = format!(r"\{command}{{Fun fact}}");
            let expected = if command == "text" { "Fun fact" } else { "Funfact" };
            assert_eq!(preview_runs(&label), runs(&[(expected, false)]));
        }
        assert_eq!(preview_runs(r"x\mathrm{im}y"), runs(&[("x", true), ("im ", false), ("y", true)]));
        assert_eq!(preview_runs(r"\operatorname{Aut}(G)"), runs(&[("Aut(", false), ("G", true), (")", false)]));
        assert_eq!(preview_runs(r"\ker\varphi"), runs(&[("ker ", false), ("φ", true)]));
    }

    #[test]
    fn text_math_islands_and_nested_groups_keep_their_styles() {
        let expected = runs(&[("kernel of ", false), ("φ", true)]);
        assert_eq!(preview_runs(r"\text{kernel of $\varphi$}"), expected);
        assert_eq!(preview_text_runs(r"kernel of $\varphi$"), expected);
        assert_eq!(preview(r"\text{kernel of $\varphi$}"), "kernel of φ");
        assert_eq!(preview_runs(r"\text{a {nested} word \$5 and $x_1$}"),
            runs(&[("a nested word $5 and ", false), ("x", true), ("₁", false)]));
        assert_eq!(preview_runs(r"\text{$x$ and $\mathrm{im}\varphi$}y"),
            runs(&[("x", true), (" and im ", false), ("φ", true), (" ", false), ("y", true)]));
    }

    #[test]
    fn digits_and_transformed_arguments_keep_styles() {
        assert_eq!(preview_runs("123"), runs(&[("123", false)]));
        assert_eq!(preview_runs(r"x_{12}+\frac{\text{one}}{y2}"),
            runs(&[("x", true), ("₁₂+one/", false), ("y", true), ("2", false)]));
        assert_eq!(preview_runs(r"\sqrt{\mathrm{ab}}+\bar{x}"),
            runs(&[("√ab+", false), ("x\u{0304}", true)]));
        assert_eq!(preview_runs("G^{2q}"), runs(&[("G", true), ("^2", false), ("q", true)]));
        assert_eq!(preview_runs("G^{ab}"), runs(&[("Gᵃᵇ", true)]));
    }

    #[test]
    fn styled_preview_tolerates_incomplete_input() {
        for s in ["", "\\", "{", "}", "^", "_", "^{", r"\frac{a}", r"\text{kernel $\varphi", r"\mathrm", r"\operatorname*", r"\text{\$}"] {
            let _ = preview_runs(s);
            let _ = preview_text_runs(s);
        }
    }
}
