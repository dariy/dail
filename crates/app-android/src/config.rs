//! What Kotlin sends: `open`'s configuration, `blob`'s request and `background`'s task.

use std::path::PathBuf;

use app_core::{DraftId, Origin};
use serde::{Deserialize, Serialize};

/// `open`'s argument.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// The profile directory (`<filesDir>/profiles/default`).
    pub profile_dir: PathBuf,
    /// The app's cache directory (temporary files).
    #[serde(default)]
    pub cache_dir: Option<PathBuf>,
    /// Where picked and shared files are copied before they are attached
    /// (`<filesDir>/outgoing`; `<profile_dir>/outgoing` if unset).
    #[serde(default)]
    pub outgoing_dir: Option<PathBuf>,
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
    /// Attachment `part` as text for the preview: UTF-8, at most 256 KiB, every line cleaned
    /// (`app_mobile::preview_text`).
    Text {
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
    /// Wait until no send is scheduled or leaving (`MessageSent`, `SendFailed` or
    /// `cancel_send`): what the send service runs.
    Sends,
}

/// `background`'s argument for a file the user picked or shared: `{"import": {…}}`. The host
/// copies it into the app (`outgoing`) and attaches the copy to `draft`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportTask {
    /// Copy and attach one file.
    Import {
        /// The draft.
        draft: DraftId,
        /// The file's display name (sanitized before use).
        name: String,
        /// A path the host can read it from: Kotlin's `/proc/self/fd/<n>` of an open descriptor.
        source: PathBuf,
    },
}

/// `background`'s argument for a notification's button: `{"notification": {"act": …}}`, the
/// button's `act` passed back verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationTask {
    /// Do what the button says.
    Notification(app_mobile::NoteAct),
}

/// `background`'s argument from the debug adb bridge (debug builds only; a release build refuses
/// it): what an agent or an instrumented test asks of the session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugTask {
    /// Submit `command` as `origin`: `cli` (as the user at a terminal) or `mcp` (as an agent,
    /// under the agent policy). Any other origin is refused.
    Exec {
        /// Who submits it.
        origin: Origin,
        /// The registry command, `{"cmd": …, "args": {…}}` (`args` may be left out when empty).
        command: serde_json::Value,
    },
    /// The whole `AppState`.
    State,
}
