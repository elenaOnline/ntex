//! Ntex Studio. See ../ARCHITECTURE.md (modules, who owns what) and ../UX.md
//! (every control, every key, the LaTeX look).
//!
//! Workspace: `core/` (ntex-studio-core: model + pure logic, CORE),
//! `io/` (ntex-studio-io: processes, BRIDGE), this crate (the GPUI app:
//! `ui/widgets` WIDGETS, `ui/screens` + `ui/app.rs` + main.rs SCREENS).
pub use ntex_studio_core as core;
pub use ntex_studio_core::model;
pub use ntex_studio_io as io;
pub mod ui;

pub use model::*;
