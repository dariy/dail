//! Mark as read after a delay (UX-5): the open conversation's unread messages are marked read
//! once it has been open for `mark_read_after_ms`.
//!
//! The timer lives in the front-end, not in the engine: the result must be an ordinary,
//! undoable `mark_read` from the user, and the engine never records history for a `Tick`.
//! Agents opening threads over MCP therefore never mark mail read.

use app_core::{AppState, Command, MessageId, ThreadId, listing};

/// What [`AutoRead::poll`] asks the front-end to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Poll {
    /// Nothing to wait for.
    Idle,
    /// Poll again in this many milliseconds (at the latest).
    Wait(u64),
    /// Submit this command (once) as the user's.
    Fire(Command),
}

/// The timer of one front-end. Call [`AutoRead::poll`] every frame (or event-loop turn) and
/// [`AutoRead::note`] with every command the user submits.
#[derive(Debug, Clone, Default)]
pub struct AutoRead {
    /// The thread the timer runs for.
    thread: Option<ThreadId>,
    /// Milliseconds it has been open while the window had the focus.
    open_ms: u64,
    /// When the last poll was.
    last_ms: Option<i64>,
    /// Fired, or disarmed by the user's `mark_unread`: nothing more until another thread opens.
    done: bool,
}

impl AutoRead {
    /// A user command: an explicit `mark_unread` disarms the timer until another thread opens,
    /// so that it never fights the user.
    pub fn note(&mut self, cmd: &Command) {
        if matches!(cmd, Command::MarkUnread { .. }) {
            self.done = true;
        }
    }

    /// Advances the timer to `now_ms`. Time counts only while `focused` (the window has the
    /// keyboard focus). Fires once per opened thread, when the delay has passed and the thread
    /// still has unread messages that are expanded in the reader.
    pub fn poll(&mut self, state: &AppState, now_ms: i64, focused: bool) -> Poll {
        let last = self.last_ms.replace(now_ms);
        if state.view.thread != self.thread {
            self.thread = state.view.thread;
            self.open_ms = 0;
            self.done = false;
        } else if focused && let Some(last) = last {
            let step = u64::try_from(now_ms.saturating_sub(last)).unwrap_or(0);
            self.open_ms = self.open_ms.saturating_add(step);
        }
        let delay = state.settings.mark_read_after_ms;
        let Some(thread) = self.thread else {
            return Poll::Idle;
        };
        if delay == 0 || self.done {
            return Poll::Idle;
        }
        let unread = unread_expanded(state, thread);
        if unread.is_empty() {
            return Poll::Idle;
        }
        if !focused {
            // Paused in the background: the focus change wakes the front-end.
            return Poll::Idle;
        }
        if self.open_ms < delay {
            return Poll::Wait(delay - self.open_ms);
        }
        self.done = true;
        Poll::Fire(Command::MarkRead {
            ids: Vec::new(),
            messages: unread,
        })
    }
}

/// The unread messages of `thread` that the reader shows expanded.
fn unread_expanded(state: &AppState, thread: ThreadId) -> Vec<MessageId> {
    let Some(thread) = state.threads.get(&thread) else {
        return Vec::new();
    };
    thread
        .messages
        .iter()
        .filter_map(|id| state.messages.get(id))
        .filter(|m| !m.flags.seen && listing::is_expanded(state, thread, m))
        .map(|m| m.id)
        .collect()
}
