//! dail's mobile host without the platform (ADR 0010, `ARCHITECTURE.md §7.5`): what Android's
//! `libdail.so` and iOS's `libdail.a` share.
//!
//! [`Host`] holds the tokio runtime, the session (the profile's live backend, or the memory
//! backend behind the `demo` feature), an `app_sync::Bridge` with `Origin::Gui`, and the
//! presenter (`app_mobile::Mobile`). [`calls`] are the six calls a native front-end makes
//! (`open`, `event`, `frame`, `blob`, `background`, `close`), JSON in and out, in safe Rust. The
//! platform crates convert their arguments and pass a [`Platform`]. Tests drive it on the desktop.

pub mod calls;
pub mod config;
pub mod error;
pub mod host;
pub mod outgoing;
pub mod platform;

pub use config::{
    BackendChoice, BlobRequest, Config, DebugTask, ImportTask, NotificationTask, Task,
};
pub use error::HostError;
pub use host::{Alert, Host, Output};
pub use platform::{DESKTOP, Platform};
