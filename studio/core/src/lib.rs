//! ntex-studio-core: the shared shapes (model.rs) and pure logic: no GPUI, no processes
//! Owner: the CORE builder. Unit-tested with `cargo test --lib`.
pub mod model;
pub mod codegen;
pub mod luaparse;
pub mod inbox;
pub mod infer;
pub mod words;
pub mod blocks;
pub mod colour;
pub mod apply;
pub mod prefs;
