//! The kit. Public API below is the contract the SCREENS builder codes
//! against: keep every name and signature; add freely.
pub mod theme;
pub mod fonts;
pub mod boxes;
pub mod controls;
pub mod text_field;
pub mod snippet_field;
pub mod trigger_field;
pub mod token_editor;
pub mod code_view;
pub mod edit;

/// Register fonts, the theme global and the widgets' key bindings.
pub fn init(cx: &mut gpui::App) {
    fonts::register(cx);
    theme::init(cx);
    text_field::bind_keys(cx);
    snippet_field::bind_keys(cx);
    trigger_field::bind_keys(cx);
    token_editor::bind_keys(cx);
}
