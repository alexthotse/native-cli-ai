//! Full-screen session TUI (transcript + streaming + composer).

pub mod app;
pub mod bridge;
pub mod state;

pub use app::{run_blocking, TuiCmd};
pub use bridge::spawn_tui_bridge;
pub use state::{DisplayBlock, TuiSessionState};
