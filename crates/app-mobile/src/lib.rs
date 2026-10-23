//! dail's touch presenter (ADR 0009, `ARCHITECTURE.md §7.5`): a projection of `AppState` into
//! JSON screen models ("frames") and of touch events into `Command`s, like the TUI with frames
//! instead of a terminal buffer.
//!
//! [`Mobile`] reads `&AppState` and never changes it, never sees the session, and holds no
//! toolkit, tokio or JNI: the Android host (`app-android`) submits its [`Request`]s and draws
//! its [`Frame`]s in Compose. Tests drive it over a real `Engine`.

pub mod composer;
pub mod event;
pub mod frame;
pub mod mobile;
pub mod notify;
pub mod request;
pub mod theme;

pub use event::{External, UiEvent};
pub use frame::{Class, Frame};
pub use mobile::{Mobile, preview_text};
pub use notify::{NoteAct, Notification, notifications, push_status};
pub use request::{Purpose, Reply, Request};
pub use theme::{About, FrameTheme};
