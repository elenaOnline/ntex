//! Everything on screen. `widgets/` (WIDGETS builder) is the LaTeX-looking
//! kit; `screens/` + `app.rs` (SCREENS builder) are the pages and the state.
pub mod widgets;
pub mod screens;
pub mod app;
/// FIG-UI: the figure window (Ntex Figures).
pub mod figure;
/// Background launch mode (never takes focus).
pub mod launch;
