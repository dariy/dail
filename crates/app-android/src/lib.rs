//! dail's Android host (ADR 0009, `ARCHITECTURE.md §7.6`), built as `libdail.so`.
//!
//! [`Host`] is the JNI-free core: the tokio runtime, the session (the profile's live backend, or
//! the memory backend behind the `demo` feature), an `app_sync::Bridge` with `Origin::Gui`, and
//! the presenter (`app_mobile::Mobile`). Kotlin's calls (`open`, `event`, `frame`, `blob`,
//! `background`, `close`) map onto its methods and carry JSON. Tests drive it on the desktop.

pub mod calls;
pub mod config;
pub mod error;
pub mod host;
mod jni;
pub mod logging;

pub use config::{BackendChoice, BlobRequest, Config, Task};
pub use error::HostError;
pub use host::{Host, Output};
