//! What a front-end tells the user about its own commands: the latest undoable action, the
//! scheduled send to count down, and the text of a rejection.

use app_core::{AppState, CommandReceipt, CoreError, DraftId};

use crate::compose;

/// The latest undoable action of this front-end, shown with *Undo (z)* until an `undo` or
/// `redo` goes through (`z` only ever undoes the latest step).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LastAction {
    label: Option<String>,
}

impl LastAction {
    /// The undo label, if there is one.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Notes the receipt of a user command. Returns the draft whose send it scheduled, if any:
    /// the front-end counts it down with *Cancel send*.
    pub fn observe(&mut self, receipt: &CommandReceipt) -> Option<DraftId> {
        if matches!(receipt.name.as_str(), "undo" | "redo") {
            // The bar would offer to undo something that is no longer the latest step.
            self.label = None;
        }
        if let Some(label) = &receipt.undo_label {
            self.label = Some(label.clone());
        }
        // An approved request is the user's send: its receipt is the send's.
        if matches!(receipt.name.as_str(), "send_draft" | "approve_request")
            && receipt.data["send_at"].is_i64()
        {
            return serde_json::from_value::<DraftId>(receipt.data["id"].clone()).ok();
        }
        None
    }
}

/// The text of a rejected command: `Archive: <the engine's error>`.
pub fn rejection(title: &str, err: &CoreError) -> String {
    format!("{title}: {err}")
}

/// Whether the send of `draft` can still be cancelled (scheduled, not yet leaving).
pub fn send_pending(state: &AppState, draft: DraftId) -> bool {
    state
        .drafts
        .get(&draft)
        .is_some_and(|d| d.send_at.is_some() && !d.sending)
}

/// `Sending in 7 s` for a scheduled send of `draft`.
pub fn sending_text(state: &AppState, draft: DraftId, now_ms: i64) -> String {
    let left = state
        .drafts
        .get(&draft)
        .and_then(|d| d.send_at)
        .map_or(0, |at| compose::seconds_left(at, now_ms));
    format!("Sending in {left} s")
}
