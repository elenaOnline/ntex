//! FIG-UI: the figure window's starting file is the shared
//! `figures/template.fig` (FIG-CORE-B's, also `ntex-fig new`). It is copied
//! into OUT_DIR so the app still builds, with a one-line fallback, if it is
//! missing.
fn main() {
    let src = std::path::Path::new("figures/template.fig");
    println!("cargo:rerun-if-changed=figures/template.fig");
    let text = std::fs::read_to_string(src).unwrap_or_else(|_| "% Ntex Figures: write the shorthand here (studio/figures/FIGURES.md).\n".into());
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("template.fig");
    std::fs::write(out, text).unwrap();
}
