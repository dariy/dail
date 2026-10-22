//! What Kotlin sends: `open`'s configuration, `blob`'s request and `background`'s task.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// `open`'s argument.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// The profile directory (`<filesDir>/profiles/default`).
    pub profile_dir: PathBuf,
    /// The app's cache directory (temporary files).
    #[serde(default)]
    pub cache_dir: Option<PathBuf>,
    /// Which backend the session runs over.
    #[serde(default)]
    pub backend: BackendChoice,
    /// The system's language tag, for the labels before the first frame.
    #[serde(default)]
    pub lang: Option<String>,
    /// The app's version name.
    #[serde(default)]
    pub version: Option<String>,
}

/// The session's backend: `"live"` or `{"memory": "<fixture>"}`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendChoice {
    /// The profile's store, the keyring and the mail servers.
    #[default]
    Live,
    /// A fake server over a built-in fixture, its first account added and synced (the `demo`
    /// feature only).
    Memory(String),
}

/// `blob`'s argument: bytes of a stored message body's part.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlobRequest {
    /// Attachment `part` of the body `blob`, as stored.
    Attachment {
        /// The body's blob key.
        blob: String,
        /// The attachment's index.
        part: usize,
    },
    /// Attachment `part`, decoded as an image (within `app_io::images`' limits).
    Image {
        /// The body's blob key.
        blob: String,
        /// The attachment's index.
        part: usize,
    },
    /// The inline part `Content-ID: <cid>`, decoded as an image.
    Inline {
        /// The body's blob key.
        blob: String,
        /// The part's Content-ID, without angle brackets.
        cid: String,
    },
}

/// `background`'s argument: work a job or a service runs while the app may be off screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Task {
    /// `sync_now` as `runtime`, then wait until no account is syncing.
    Check,
    /// Wait until the queued work (remote ops, draft saves, attachment saves) is done.
    Flush,
}
