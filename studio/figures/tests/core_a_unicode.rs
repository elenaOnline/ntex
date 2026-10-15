//! FIG-CORE-A: the Unicode table, both ways.
use ntex_figures::unicode::{preview, to_latex, to_latex_text};

#[test]
fn greek_with_her_var_forms() {
    assert_eq!(to_latex("φ"), r"\varphi");
    assert_eq!(to_latex("ε"), r"\varepsilon");
    assert_eq!(to_latex("ϕ"), r"\phi");
    assert_eq!(to_latex("ϵ"), r"\epsilon");
    assert_eq!(to_latex("αβγδ"), r"\alpha\beta\gamma\delta");
    assert_eq!(to_latex("ΓΔΘΛΞΠΣΦΨΩ"), r"\Gamma\Delta\Theta\Lambda\Xi\Pi\Sigma\Phi\Psi\Omega");
    // Greek capitals that look Latin are Latin
    assert_eq!(to_latex("ΑΒ"), "AB");
}

#[test]
fn control_words_get_a_space_before_letters() {
    assert_eq!(to_latex("φx"), r"\varphi x");
    assert_eq!(to_latex("φ(G)"), r"\varphi(G)");
    assert_eq!(to_latex("A≅B"), r"A\cong B");
    assert_eq!(to_latex("φψ"), r"\varphi\psi");
    assert_eq!(to_latex("ℤx"), r"\mathbb{Z}x");
}

#[test]
fn blackboard_letters() {
    assert_eq!(to_latex("ℤ/nℤ"), r"\mathbb{Z}/n\mathbb{Z}");
    assert_eq!(to_latex("ℚℝℂℕ"), r"\mathbb{Q}\mathbb{R}\mathbb{C}\mathbb{N}");
}

#[test]
fn the_spec_operator_list() {
    let cases = [
        ("∘", r"\circ"), ("×", r"\times"), ("⊗", r"\otimes"), ("⊕", r"\oplus"), ("≅", r"\cong"),
        ("⊂", r"\subset"), ("⊆", r"\subseteq"), ("∈", r"\in"), ("∉", r"\notin"), ("∩", r"\cap"),
        ("∪", r"\cup"), ("∅", r"\varnothing"), ("∞", r"\infty"), ("≤", r"\leq"), ("≥", r"\geq"),
        ("≠", r"\neq"), ("↦", r"\mapsto"), ("→", r"\to"),
    ];
    for (u, l) in cases {
        assert_eq!(to_latex(u), l, "{u}");
    }
}

#[test]
fn sub_and_superscripts() {
    assert_eq!(to_latex("x₁"), "x_{1}");
    assert_eq!(to_latex("g²"), "g^{2}");
    assert_eq!(to_latex("x₁₂"), "x_{12}");
    assert_eq!(to_latex("g⁻¹"), "g^{-1}");
    assert_eq!(to_latex("Gᵃᵇ"), "G^{ab}");
    assert_eq!(to_latex("aₙ₊₁"), "a_{n+1}");
    assert_eq!(to_latex("H′"), "H'");
}

#[test]
fn ascii_and_latex_pass_through() {
    for s in [r"\ker\varphi", "G/K", r"\mathbb{Z}", "x^{-1}", "A -> B", ""] {
        assert_eq!(to_latex(s), s);
    }
    // so it is idempotent
    let once = to_latex("φ(G) ≅ ℤ/nℤ");
    assert_eq!(to_latex(&once), once);
    assert_eq!(once, r"\varphi(G) \cong \mathbb{Z}/n\mathbb{Z}");
}

#[test]
fn text_gets_maths_islands() {
    assert_eq!(to_latex_text("kernel ↦ identity"), r"kernel $\mapsto$ identity");
    assert_eq!(to_latex_text("plain"), "plain");
    assert_eq!(to_latex_text("φ: G → H"), r"$\varphi$: G $\to$ H");
}

#[test]
fn preview_typical_labels() {
    assert_eq!(preview(r"\ker\varphi"), "ker φ");
    assert_eq!(preview(r"\mathbb{Z}/n\mathbb{Z}"), "ℤ/nℤ");
    assert_eq!(preview("G^{ab}"), "Gᵃᵇ");
    assert_eq!(preview(r"\varphi(G)_{1}"), "φ(G)₁");
    assert_eq!(preview(r"g \circ f"), "g ∘ f");
    assert_eq!(preview(r"A \cong B"), "A ≅ B");
    assert_eq!(preview("g^{-1}"), "g⁻¹");
    assert_eq!(preview("x^2"), "x²");
    assert_eq!(preview(r"\mathbb{F}_p"), "𝔽ₚ");
    assert_eq!(preview(r"\operatorname{Aut}(G)"), "Aut(G)");
    assert_eq!(preview(r"\mathrm{im}\,\varphi"), "im φ");
    assert_eq!(preview(r"\langle a \rangle"), "⟨a⟩");
    assert_eq!(preview(r"\left( x \right)"), "(x)");
    assert_eq!(preview(r"\{e\}"), "{e}");
    assert_eq!(preview(r"H \trianglelefteq G"), "H ⊴ G");
    assert_eq!(preview(r"\text{a b}"), "a b");
}

#[test]
fn preview_falls_back_readably() {
    // no superscript q exists
    assert_eq!(preview("G^{q}"), "G^q");
    assert_eq!(preview("G^{2q}"), "G^2q");
    assert_eq!(preview("x_{i,j}"), "x_(i,j)");
    // unknown commands lose only the backslash
    assert_eq!(preview(r"\foo"), "foo");
    assert_eq!(preview(r"\frac{a}{b}"), "a/b");
}

#[test]
fn preview_inverts_the_input_table() {
    for u in ["φ", "ε", "ℤ", "≅", "∘", "↦", "Γ", "∅", "⊗"] {
        assert_eq!(preview(&to_latex(u)), u, "{u}");
    }
}

#[test]
fn preview_never_panics() {
    for s in ["", "\\", "{", "}", "^", "_", "^{", r"\frac{a}", r"\mathbb", "x^", r"\left", "{{{}}}", r"\\", "$x$", "~"] {
        let _ = preview(s);
    }
}
