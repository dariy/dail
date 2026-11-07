//! Notifications while no activity is visible (Wave 4 A5): `app_front::alerts` as JSON for the
//! host's alert watcher, which hands each one to Kotlin (`Listener.alert`). Kotlin posts them
//! and passes an action's [`NoteAct`] back verbatim; [`NoteAct::commands`] decides what it does,
//! so the rules stay here.
//!
//! Texts come from `app_front::alerts` (cleaned, short). The lock-screen version
//! (`public_title`) names no sender and no subject.

use app_core::{
    AppState, Command, DraftId, MailboxId, MessageId, SettingKey, ThreadId, WatchState,
};
use app_front::alerts::{Alert, AlertKind, alerts, new_mail_by_conversation};
use app_front::i18n::{self, Key};
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
    /// The group it shows in (new mail: `new_mail`, one notification per conversation).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Whether it is its group's summary (Android shows it in place of the group when it folds).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub summary: bool,
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
    /// Archive these threads, or only these messages when `messages` is not empty.
    Archive {
        /// The threads.
        threads: Vec<ThreadId>,
        /// The messages (a conversation's new ones).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        messages: Vec<MessageId>,
    },
    /// Mark these threads read, or only these messages when `messages` is not empty.
    MarkRead {
        /// The threads.
        threads: Vec<ThreadId>,
        /// The messages (a conversation's new ones).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        messages: Vec<MessageId>,
    },
    /// Move these threads to Trash, or only these messages when `messages` is not empty.
    Trash {
        /// The threads.
        threads: Vec<ThreadId>,
        /// The messages (a conversation's new ones).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        messages: Vec<MessageId>,
    },
    /// Open this message: its mailbox, its thread in the reader, the message expanded.
    OpenMessage {
        /// The mailbox.
        mailbox: MailboxId,
        /// The thread.
        thread: ThreadId,
        /// The message.
        message: MessageId,
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
            Self::Archive { threads, messages } => scoped(threads, messages, state)
                .map(|(ids, messages)| Command::ArchiveThreads { ids, messages })
                .into_iter()
                .collect(),
            Self::MarkRead { threads, messages } => scoped(threads, messages, state)
                .map(|(ids, messages)| Command::MarkRead { ids, messages })
                .into_iter()
                .collect(),
            Self::Trash { threads, messages } => scoped(threads, messages, state)
                .map(|(ids, messages)| Command::TrashThreads { ids, messages })
                .into_iter()
                .collect(),
            Self::OpenMessage {
                mailbox,
                thread,
                message,
            } if state.threads.contains_key(thread) => {
                let mut out = Vec::new();
                if state.mailboxes.contains_key(mailbox) {
                    out.push(Command::OpenMailbox { id: *mailbox });
                }
                out.push(Command::OpenThread { id: *thread });
                if state
                    .messages
                    .get(message)
                    .is_some_and(|m| m.thread == *thread)
                {
                    out.push(Command::ExpandMessages {
                        ids: vec![*message],
                    });
                }
                out
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
            Self::OpenMessage { .. }
            | Self::OpenThread { .. }
            | Self::OpenDraft { .. }
            | Self::BatterySettings => Vec::new(),
        }
    }
}

/// What a triage act targets: the messages that still exist when it names messages (never the
/// rest of their threads), else the threads that still exist. `None` when nothing is left.
fn scoped(
    threads: &[ThreadId],
    messages: &[MessageId],
    state: &AppState,
) -> Option<(Vec<ThreadId>, Vec<MessageId>)> {
    let target = if messages.is_empty() {
        (known(threads, state), Vec::new())
    } else {
        let left: Vec<MessageId> = messages
            .iter()
            .filter(|m| state.messages.contains_key(*m))
            .copied()
            .collect();
        (Vec::new(), left)
    };
    (!target.0.is_empty() || !target.1.is_empty()).then_some(target)
}

fn known(threads: &[ThreadId], state: &AppState) -> Vec<ThreadId> {
    threads
        .iter()
        .filter(|t| state.threads.contains_key(*t))
        .copied()
        .collect()
}

/// The group of the new-mail notifications.
pub const NEW_MAIL_GROUP: &str = "new_mail";

/// The notifications for going from `before` to `after` (none when `notifications` is 0): one
/// per new conversation (tag `new_mail:<thread>`, group `new_mail`) with *Delete*, *Mark read*
/// and *Archive* on its new messages and a tap that opens the newest one, then the group's
/// summary (tag `new_mail`); then one per failed send. The lock screen shows only *New mail*.
/// Approval requests are left out: the phone has no approvals (ADR 0009).
pub fn notifications(before: &AppState, after: &AppState) -> Vec<Notification> {
    let conversations = new_mail_by_conversation(before, after);
    let count = conversations.len();
    let mut out: Vec<Notification> = conversations.into_iter().filter_map(conversation).collect();
    if count > 0 {
        let title = if count == 1 {
            i18n::s(Key::NewMail).to_owned()
        } else {
            i18n::f(Key::NewConversations, &[("n", &count.to_string())])
        };
        out.push(Notification {
            tag: NEW_MAIL_GROUP.to_owned(),
            channel: Channel::NewMail,
            title,
            body: String::new(),
            public_title: i18n::s(Key::NewMail).to_owned(),
            open: None,
            actions: Vec::new(),
            group: Some(NEW_MAIL_GROUP.to_owned()),
            summary: true,
        });
    }
    out.extend(alerts(before, after).into_iter().filter_map(|alert| {
        match alert.kind {
            AlertKind::SendFailed => Some(Notification {
                tag: alert
                    .draft
                    .map_or_else(|| "not_sent".to_owned(), |d| format!("not_sent:{d}")),
                channel: Channel::NotSent,
                public_title: i18n::s(Key::AMessageWasNotSent).to_owned(),
                title: alert.title,
                body: alert.body,
                open: alert.draft.map(|draft| NoteAct::OpenDraft { draft }),
                actions: Vec::new(),
                group: None,
                summary: false,
            }),
            AlertKind::NewMail | AlertKind::ApprovalNeeded => None,
        }
    }));
    out
}

/// One new conversation's notification.
fn conversation(alert: Alert) -> Option<Notification> {
    let thread = *alert.threads.first()?;
    let open = match (alert.mailbox, alert.messages.last()) {
        (Some(mailbox), Some(&message)) => Some(NoteAct::OpenMessage {
            mailbox,
            thread,
            message,
        }),
        _ => Some(NoteAct::OpenThread { thread }),
    };
    let threads = vec![thread];
    let messages = alert.messages;
    Some(Notification {
        tag: format!("{NEW_MAIL_GROUP}:{thread}"),
        channel: Channel::NewMail,
        public_title: i18n::s(Key::NewMail).to_owned(),
        title: alert.title,
        body: alert.body,
        open,
        actions: vec![
            NoteAction {
                label: i18n::s(Key::NoteDelete).to_owned(),
                act: NoteAct::Trash {
                    threads: threads.clone(),
                    messages: messages.clone(),
                },
            },
            NoteAction {
                label: i18n::s(Key::MarkRead).to_owned(),
                act: NoteAct::MarkRead {
                    threads: threads.clone(),
                    messages: messages.clone(),
                },
            },
            NoteAction {
                label: i18n::s(Key::Archive).to_owned(),
                act: NoteAct::Archive { threads, messages },
            },
        ],
        group: Some(NEW_MAIL_GROUP.to_owned()),
        summary: false,
    })
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
        body: i18n::s(Key::NewMailArrivesAsItComes).to_owned(),
        open: None,
        actions: vec![
            NoteAction {
                label: i18n::s(Key::Stop).to_owned(),
                act: NoteAct::StopPush,
            },
            NoteAction {
                label: i18n::s(Key::Battery).to_owned(),
                act: NoteAct::BatterySettings,
            },
        ],
        group: None,
        summary: false,
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
        0 => i18n::s(Key::ConnectingToYourInboxes).to_owned(),
        1 => i18n::s(Key::Watching1Inbox).to_owned(),
        n => i18n::f(Key::WatchingInboxes, &[("n", &n.to_string())]),
    })
}
