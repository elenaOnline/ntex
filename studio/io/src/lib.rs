//! Processes and the outside world: the headless ntex bridge, tectonic
//! renders, the sorting agent, the single-instance socket.
//! Owner: the BRIDGE builder. Nothing here may open a window or touch a viewer.
pub mod bridge;
pub mod render;
pub mod agent;
pub mod instance;
pub mod figure_ai;
