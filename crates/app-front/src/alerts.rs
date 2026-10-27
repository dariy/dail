//! Desktop notifications: what changed between two states that the user should hear about while
//! dail is in the background — new unread mail in an INBOX, a send that failed, an agent's
//! message waiting for approval. The front-end decides when to ask (e.g. only while its window
//! is not focused) and how to show them; nothing here talks to the desktop.
//!
//! Senders and subjects are untrusted: the texts are cleaned of control characters and cut short,
//! and the delivery must still escape them for its format (markup, XML).

use app_core::{AppState, DraftId, FocusTarget, MailboxRole, ThreadId};

use crate::format;

/// Longest title or body, in characters.
const MAX_CHARS: usize = 120;
/// Senders named in the body of a notification for several new messages.
const MAX_SENDERS: usize = 3;

/// What a notification is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertKind {
    /// New unread mail.
    NewMail,
    /// A message could not be sent.
    SendFailed,
    /// An agent's message waits for the user's approval.
    ApprovalNeeded,
}

/// One desktop notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alert {
    /// What it is about.
    pub kind: AlertKind,
    /// The headline (cleaned, short).
    pub title: String,
    /// The text below it (cleaned, short).
    pub body: String,
    /// The threads it is about (new mail), oldest first, each once: a tap on a notification
    /// for one thread opens it, and its actions (archive, mark read) act on them all.
    pub threads: Vec<ThreadId>,
    /// The draft it is about (a failed send, an approval request): the first one.
    pub draft: Option<DraftId>,
}

/// The notifications for going from `before` to `after`, at most one of each kind. None when
/// the user turned them off (`notifications` setting 0).
pub fn alerts(before: &AppState, after: &AppState) -> Vec<Alert> {
    if after.settings.notifications == 0 {
        return Vec::new();
    }
    [
        new_mail(before, after),
        send_failed(before, after),
        approvals(before, after),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Unread messages that arrived in an INBOX (in focus mode: the focused mailbox, or the
/// focused account's inbox) already fetched before (a first sync or a history
/// backfill is not news): new to the state and above the INBOX's `uid_next` of `before`.
fn new_mail(before: &AppState, after: &AppState) -> Option<Alert> {
    let is_inbox =
        |id| after.mailboxes.get(&id).map(|mb| mb.role) == Some(Some(MailboxRole::Inbox));
    let fresh: Vec<_> = after
        .messages
        .values()
        .filter(|m| !m.flags.seen && !before.messages.contains_key(&m.id))
        .filter(|m| {
            let watched = match after.view.focused {
                // Focus mode (UX-4): only its mailbox, or its account's inbox.
                Some(FocusTarget::Mailbox(id)) => m.mailbox == id,
                Some(FocusTarget::Account(id)) => m.account == id && is_inbox(m.mailbox),
                None => is_inbox(m.mailbox),
            };
            watched
                && before
                    .mailboxes
                    .get(&m.mailbox)
                    .and_then(|mb| mb.uid_next)
                    .is_some_and(|next| m.uid >= next)
        })
        .collect();
    if fresh.is_empty() {
        return None;
    }
    let mut threads: Vec<ThreadId> = Vec::new();
    for m in &fresh {
        if !threads.contains(&m.thread) {
            threads.push(m.thread);
        }
    }
    let sender = |m: &app_core::Envelope| {
        m.from.first().map_or_else(
            || "(unknown sender)".to_owned(),
            |a| format::name(a).to_owned(),
        )
    };
    Some(if let [only] = fresh[..] {
        Alert {
            kind: AlertKind::NewMail,
            title: clean(&sender(only)),
            body: clean(if only.subject.trim().is_empty() {
                "(no subject)"
            } else {
                &only.subject
            }),
            threads,
            draft: None,
        }
    } else {
        let mut senders: Vec<String> = Vec::new();
        for m in &fresh {
            let name = sender(m);
            if !senders.contains(&name) {
                senders.push(name);
            }
        }
        let more = senders.len().saturating_sub(MAX_SENDERS);
        senders.truncate(MAX_SENDERS);
        let mut body = senders.join(", ");
        if more > 0 {
            body.push_str(&format!(" and {more} more"));
        }
        Alert {
            kind: AlertKind::NewMail,
            title: format!("{} new messages", fresh.len()),
            body: clean(&body),
            threads,
            draft: None,
        }
    })
}

/// Drafts whose send failed since `before`.
fn send_failed(before: &AppState, after: &AppState) -> Option<Alert> {
    let failed: Vec<_> = after
        .drafts
        .values()
        .filter(|d| {
            d.send_error.is_some()
                && before.drafts.get(&d.id).map(|b| &b.send_error) != Some(&d.send_error)
        })
        .collect();
    let first = failed.first()?;
    let subject = if first.subject.trim().is_empty() {
        "(no subject)"
    } else {
        &first.subject
    };
    let error = first.send_error.as_deref().unwrap_or_default();
    Some(Alert {
        kind: AlertKind::SendFailed,
        title: if failed.len() == 1 {
            "Sending failed".to_owned()
        } else {
            format!("{} messages could not be sent", failed.len())
        },
        body: clean(&format!("{subject}: {error}")),
        threads: Vec::new(),
        draft: Some(first.id),
    })
}

/// Approval requests that are new since `before`.
fn approvals(before: &AppState, after: &AppState) -> Option<Alert> {
    let new: Vec<_> = after
        .approvals
        .values()
        .filter(|a| !before.approvals.contains_key(&a.id))
        .collect();
    let first = new.first()?;
    let body = after
        .drafts
        .get(&first.draft)
        .map_or_else(String::new, |d| {
            let to: Vec<_> = d.to.iter().collect();
            format!("To {}: {}", format::names(&to), d.subject)
        });
    Some(Alert {
        kind: AlertKind::ApprovalNeeded,
        title: if new.len() == 1 {
            "An agent wants to send a message".to_owned()
        } else {
            format!("Agents want to send {} messages", new.len())
        },
        body: clean(&body),
        threads: Vec::new(),
        draft: Some(first.draft),
    })
}

/// One line without control characters, at most [`MAX_CHARS`] long.
fn clean(text: &str) -> String {
    let line: String = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| {
            !c.is_control() && !matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
        .collect();
    if line.chars().count() > MAX_CHARS {
        let mut cut: String = line.chars().take(MAX_CHARS - 1).collect();
        cut.push('…');
        cut
    } else {
        line
    }
}
