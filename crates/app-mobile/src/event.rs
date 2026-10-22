//! What the host sends in (`UiEvent`) and what it is asked to do (`External`).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
}
