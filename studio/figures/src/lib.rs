//! Ntex Figures. FIGURES.md is the spec. `model` is shared and add-only.
pub mod model;
pub mod unicode;   // FIG-CORE-A
pub mod parse;     // FIG-CORE-A
pub mod format;    // FIG-CORE-A
pub mod edit;      // FIG-CORE-A
pub mod layout;    // FIG-CORE-B
pub mod cayley;    // FIG-CORE-B
pub mod tikz;      // FIG-CORE-B
pub mod recognize; // FIG-CORE-B
pub mod build;     // FIG-CORE-B
pub mod spelling;  // FIG-POLISH
pub mod clip;      // figure-qol A: copy, paste and clone objects
pub(crate) mod gridedit; // FIG-INTERACT: grid blocks edited in place
pub mod labeltext; // Text-mode snippet fields
