//! dail terminal UI (ratatui): a projection of `AppState` and an emitter of `Command`s, like the
//! GUI, for SSH sessions from a phone or a tablet (`ARCHITECTURE.md §7.4`, ADR 0004).
//!
//! [`Tui`] draws one frame from `&AppState` and turns input events into [`Request`]s; it never
//! sees the session. [`run`] owns the terminal and connects the two through an
//! `app_sync::Bridge`, so tests drive [`Tui`] headlessly with ratatui's `TestBackend`.

pub mod approvals;
pub mod composer;
pub mod editor;
pub mod keys;
pub mod layout;
pub mod palette;
pub mod reader;
pub mod request;
pub mod run;
pub mod settings;
pub mod setup;
pub mod text;
pub mod tui;
pub mod view;

pub use request::{Purpose, Reply, Request};
pub use run::{Event, Exit, Host, Target, event_loop, run};
pub use tui::{EditOutcome, External, Input, Tui};

/// TUI failure.
#[derive(Debug, thiserror::Error)]
pub enum TuiError {
    /// The terminal could not be set up, read or drawn.
    #[error("terminal: {0}")]
    Terminal(#[from] std::io::Error),
    /// The session failed.
    #[error(transparent)]
    Session(#[from] app_sync::SyncError),
}
