//! Full-screen session TUI (transcript + streaming + composer).

pub mod app;
pub mod bridge;
pub mod replay;
pub mod state;

pub use app::{TuiCmd, run_blocking};
pub use bridge::spawn_tui_bridge;
pub use replay::replay_event_log_into_state;
pub use state::{DisplayBlock, TuiSessionState};
