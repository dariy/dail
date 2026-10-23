//! What the host sends in (`UiEvent`) and what it is asked to do (`External`).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use app_core::DraftId;

use crate::frame::ActionId;

/// A text field of the composer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Field {
    /// `To` recipients.
    To,
    /// `Cc` recipients.
    Cc,
    /// `Bcc` recipients.
    Bcc,
    /// The subject.
    Subject,
    /// The body.
    Body,
    /// The palette's input.
    Query,
}

/// One event from the host, in the order the user caused it. Kotlin never builds a command: it
/// returns the action IDs of the frame it shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "event", content = "args", rename_all = "snake_case")]
pub enum UiEvent {
    /// The window and the system settings, at start and whenever they change.
    Configure {
        /// Window width in dp.
        width_dp: u32,
        /// Window height in dp.
        height_dp: u32,
        /// The system's dark mode.
        #[serde(default)]
        dark: bool,
        /// The system's font scale (1.0 = default).
        #[serde(default = "one")]
        font_scale: f32,
        /// The system's language tag (`en-US`; `en-XA` is the pseudo-locale).
        #[serde(default)]
        locale: Option<String>,
        /// A hardware keyboard is attached.
        #[serde(default)]
        keyboard: bool,
    },
    /// The app came on screen (`true`) or left it.
    Visible {
        /// Whether an activity is visible.
        visible: bool,
    },
    /// A tap on something carrying an action ID.
    Tap {
        /// The action.
        action: ActionId,
    },
    /// A long press on something carrying a long-press action ID.
    LongPress {
        /// The action.
        action: ActionId,
    },
    /// A row swiped past its threshold (the row's `swipe_start` or `swipe_end` action).
    Swipe {
        /// The action.
        action: ActionId,
    },
    /// The thread list scrolled: rows `first..first + count` are visible.
    Scrolled {
        /// The first visible row.
        first: usize,
        /// How many rows are visible.
        count: usize,
    },
    /// System Back (only sent while the frame's `back` is true).
    Back,
    /// Pull to refresh.
    Refresh,
    /// A network came back while the app is visible: syncs, as a pull would (nothing while the
    /// app is off screen or has no account).
    Online,
    /// A tap on a notification: what its `open` said, passed back verbatim (opens the thread
    /// or the draft, if it still exists).
    Notification {
        /// The notification's `open`.
        act: crate::notify::NoteAct,
    },
    /// A text field changed (the whole text, as the IME left it).
    Text {
        /// The field.
        field: Field,
        /// Its text.
        text: String,
    },
    /// The focus left a text field.
    Blur {
        /// The field.
        field: Field,
    },
    /// A key chord from a hardware keyboard, in the keymap's notation (`Ctrl+Z`, `j`, `#`).
    Key {
        /// The chord.
        chord: String,
    },
    /// The clock moved on (sent every second while the frame's `wants_tick` is true).
    Tick,
    /// Another app opened a `mailto:` link or shared text or files with dail: a new draft from
    /// the current account opens in the composer. It is never sent. Only `content://` files of
    /// other apps are attached (copied first); `file://` URIs and dail's own are refused.
    Share {
        /// The `mailto:` URI, if it was one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mailto: Option<String>,
        /// Addresses (`EXTRA_EMAIL`).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        to: Vec<String>,
        /// The subject (`EXTRA_SUBJECT`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subject: Option<String>,
        /// The text (`EXTRA_TEXT`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        /// The shared files' URIs (`EXTRA_STREAM`).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        files: Vec<String>,
    },
}

const fn one() -> f32 {
    1.0
}

/// Something only the host can do; the presenter asks for it after an event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "external", content = "args", rename_all = "snake_case")]
pub enum External {
    /// A short vibration, after a swipe that acted.
    Haptic,
    /// Open the system file picker (several files). The host copies each picked file into the
    /// app and attaches the copy to `draft` (`app-android`'s `import`).
    PickFiles {
        /// The draft the files go to.
        draft: DraftId,
    },
    /// Copy these shared files (content URIs that passed the presenter's check) into the app and
    /// attach them to `draft`, as `pick_files` does with picked ones.
    ImportFiles {
        /// The draft.
        draft: DraftId,
        /// The content URIs.
        uris: Vec<String>,
    },
    /// Open this `http` or `https` URL in the browser (the user saw it in full and chose
    /// *Open*).
    OpenUrl {
        /// The URL.
        url: String,
    },
    /// Save attachments of a stored message through the Storage Access Framework: one file
    /// with the system's "create document" picker, several into a folder the user picks.
    /// Nothing is overwritten (the documents provider gives a taken name a suffix). The host
    /// reads each file with `blob`'s `attachment` request.
    SaveFiles {
        /// The body's blob key.
        blob: String,
        /// The files.
        files: Vec<SaveFile>,
    },
    /// Put this text on the clipboard.
    Copy {
        /// The text.
        text: String,
    },
    /// A send was scheduled: keep the process alive (a foreground service running the
    /// `sends` task) until no send is scheduled or leaving, so leaving the app never loses it.
    Sending {
        /// The draft being sent.
        draft: DraftId,
    },
    /// The background schedule (`background_sync`, `background_interval_mins`), with the first
    /// frame and whenever it changes: the host keeps a periodic check unless `sync` is 0
    /// (2 adds push).
    Schedule {
        /// `background_sync`: 0 only while open, 1 periodic, 2 push.
        sync: u64,
        /// `background_interval_mins`.
        interval_mins: u64,
    },
}

/// One attachment to save.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SaveFile {
    /// The MIME part.
    pub part: usize,
    /// The file name to suggest: the host makes it safe (`app_io::mime::safe_file_name`)
    /// before Kotlin sees it.
    pub name: String,
    /// The MIME type.
    pub mime: String,
}
