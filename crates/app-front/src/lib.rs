//! The toolkit-free half of dail's front-ends (ADR 0004): rules that the GUI (`app-gui`) and the
//! terminal UI (`app-tui`) share, so that neither re-derives the other's.
//!
//! Everything here reads `&AppState` (or a receipt) and returns values or `Command`s; nothing
//! draws, and nothing talks to a session.

pub mod a11y;
pub mod advance;
pub mod alerts;
pub mod approvals;
pub mod autoread;
pub mod body;
pub mod compose;
pub mod fold;
pub mod format;
pub mod highlight;
pub mod i18n;
pub mod keymap;
pub mod mailbox;
pub mod mailto;
pub mod notice;
pub mod palette;
pub mod panes;
pub mod result;
pub mod status;
pub mod text;
pub mod updates;
