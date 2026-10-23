//! Notifications while no activity is visible (Wave 4 A5): `app_front::alerts` as JSON for the
//! host's alert watcher, which hands each one to Kotlin (`Listener.alert`). Kotlin posts them
//! and passes an action's [`NoteAct`] back verbatim; [`NoteAct::commands`] decides what it does,
//! so the rules stay here.
//!
//! Texts come from `app_front::alerts` (cleaned, short). The lock-screen version
//! (`public_title`) names no sender and no subject.

use app_core::{AppState, Command, DraftId, SettingKey, ThreadId, WatchState};
use app_front::alerts::{AlertKind, alerts};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The notification channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    /// *New mail*.
    NewMail,
    /// *Not sent*: a send failed.
    NotSent,
    /// *Background*: the push service's silent notification.
    Background,
}

/// One notification to post.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Notification {
    /// Posting again with the same tag replaces it.
    pub tag: String,
    /// The channel.
    pub channel: Channel,
    /// The headline (cleaned; may name the sender).
    pub title: String,
    /// The text below it (cleaned; may hold the subject).
    pub body: String,
    /// The lock screen's headline: no sender, no subject.
    pub public_title: String,
    /// What a tap does (`None`: just open the app).
    pub open: Option<NoteAct>,
    /// The buttons.
    pub actions: Vec<NoteAction>,
}

/// A button of a notification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NoteAction {
    /// Its label.
    pub label: String,
    /// What it does: Kotlin passes it back to `background` as `{"notification": …}`.
    pub act: NoteAct,
}

/// What a tap or a button of a notification does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "act", rename_all = "snake_case")]
pub enum NoteAct {
    /// Archive these threads.
    Archive {
        /// The threads.
        threads: Vec<ThreadId>,
    },
    /// Mark these threads read.
    MarkRead {
        /// The threads.
        threads: Vec<ThreadId>,
    },
    /// Open this thread in the reader.
    OpenThread {
        /// The thread.
        thread: ThreadId,
    },
    /// Open this draft in the composer.
    OpenDraft {
        /// The draft.
        draft: DraftId,
    },
    /// Stop push: `background_sync` back to 1 (periodic checks).
    StopPush,
    /// Open the system's battery-optimization settings (a host action; no command).
    BatterySettings,
}

impl NoteAct {
    /// The commands it submits, as the user. Threads or drafts that are gone submit nothing.
    pub fn commands(&self, state: &AppState) -> Vec<Command> {
        match self {
            Self::Archive { threads } => {
                let ids = known(threads, state);
                if ids.is_empty() {
                    Vec::new()
                } else {
                    vec![Command::ArchiveThreads {
                        ids,
                        messages: Vec::new(),
                    }]
                }
            }
            Self::MarkRead { threads } => {
                let ids = known(threads, state);
                if ids.is_empty() {
                    Vec::new()
                } else {
                    vec![Command::MarkRead {
                        ids,
                        messages: Vec::new(),
                    }]
                }
            }
            Self::OpenThread { thread } if state.threads.contains_key(thread) => {
                vec![Command::OpenThread { id: *thread }]
            }
            Self::OpenDraft { draft } if state.drafts.contains_key(draft) => {
                vec![Command::OpenComposer { id: *draft }]
            }
            Self::StopPush => vec![Command::SetSetting {
                key: SettingKey::BackgroundSync,
                value: 1.into(),
            }],
            Self::OpenThread { .. } | Self::OpenDraft { .. } | Self::BatterySettings => Vec::new(),
        }
    }
}

fn known(threads: &[ThreadId], state: &AppState) -> Vec<ThreadId> {
    threads
        .iter()
        .filter(|t| state.threads.contains_key(*t))
        .copied()
        .collect()
}

/// The notifications for going from `before` to `after` (none when `notifications` is 0).
/// Approval requests are left out: the phone has no approvals (ADR 0009).
pub fn notifications(before: &AppState, after: &AppState) -> Vec<Notification> {
    alerts(before, after)
        .into_iter()
        .filter_map(|alert| match alert.kind {
            AlertKind::NewMail => {
                let count = alert.threads.len();
                let open = match alert.threads[..] {
                    [thread] => Some(NoteAct::OpenThread { thread }),
                    _ => None,
                };
                Some(Notification {
                    tag: "new_mail".to_owned(),
                    channel: Channel::NewMail,
                    public_title: if count == 1 {
                        "New mail".to_owned()
                    } else {
                        format!("New mail in {count} conversations")
                    },
                    title: alert.title,
                    body: alert.body,
                    open,
                    actions: vec![
                        NoteAction {
                            label: "Archive".to_owned(),
                            act: NoteAct::Archive {
                                threads: alert.threads.clone(),
                            },
                        },
                        NoteAction {
                            label: "Mark read".to_owned(),
                            act: NoteAct::MarkRead {
                                threads: alert.threads,
                            },
                        },
                    ],
                })
            }
            AlertKind::SendFailed => Some(Notification {
                tag: alert
                    .draft
                    .map_or_else(|| "not_sent".to_owned(), |d| format!("not_sent:{d}")),
                channel: Channel::NotSent,
                public_title: "A message was not sent".to_owned(),
                title: alert.title,
                body: alert.body,
                open: alert.draft.map(|draft| NoteAct::OpenDraft { draft }),
                actions: Vec::new(),
            }),
            AlertKind::ApprovalNeeded => None,
        })
        .collect()
}

/// The tag of the push service's notification.
pub const PUSH_TAG: &str = "push";

/// The push service's silent notification (`background_sync` 2), when its text changes from
/// `before` to `after`: *Watching N inboxes* (the accounts whose INBOX watch is up), with *Stop*
/// and *Battery*. `None` when nothing changed or push is off.
pub fn push_status(before: &AppState, after: &AppState) -> Option<Notification> {
    let text = push_text(after)?;
    if push_text(before).as_ref() == Some(&text) {
        return None;
    }
    Some(Notification {
        tag: PUSH_TAG.to_owned(),
        channel: Channel::Background,
        public_title: text.clone(),
        title: text,
        body: "New mail arrives as it comes".to_owned(),
        open: None,
        actions: vec![
            NoteAction {
                label: "Stop".to_owned(),
                act: NoteAct::StopPush,
            },
            NoteAction {
                label: "Battery".to_owned(),
                act: NoteAct::BatterySettings,
            },
        ],
    })
}

fn push_text(state: &AppState) -> Option<String> {
    if state.settings.background_sync != 2 {
        return None;
    }
    let watching = state
        .status
        .sync
        .values()
        .filter(|s| s.watch == WatchState::Watching)
        .count();
    Some(match watching {
        0 => "Connecting to your inboxes".to_owned(),
        1 => "Watching 1 inbox".to_owned(),
        n => format!("Watching {n} inboxes"),
    })
}
