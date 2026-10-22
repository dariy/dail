//! Accessible names: what a screen reader says for a row. AccessKit (the GUI) and TalkBack (the
//! mobile frames) read the same names.

use app_core::ThreadSummary;

use crate::format;

/// What a screen reader says for a thread row: what the dot, the star, the clip and the tick
/// show, then senders, subject, size of the conversation and date.
pub fn thread_row(summary: &ThreadSummary<'_>, checked: bool, now_ms: i64) -> String {
    let mut parts: Vec<String> = Vec::new();
    if checked {
        parts.push("checked".to_owned());
    }
    if summary.unread > 0 {
        parts.push("unread".to_owned());
    }
    if summary.flagged {
        parts.push("starred".to_owned());
    }
    if summary.has_attachments {
        parts.push("with attachments".to_owned());
    }
    let senders = format::names(&summary.senders);
    parts.push(if senders.is_empty() {
        "(no sender)".to_owned()
    } else {
        senders
    });
    parts.push(if summary.thread.subject.trim().is_empty() {
        "(no subject)".to_owned()
    } else {
        summary.thread.subject.clone()
    });
    if summary.messages.len() > 1 {
        parts.push(format!("{} messages", summary.messages.len()));
    }
    parts.push(format::short_date(summary.thread.last_date_ms, now_ms));
    parts.join(", ")
}

/// What a screen reader says for a mailbox (or other nav) row: the label and the unread count.
pub fn nav_row(label: &str, unread: u32) -> String {
    if unread > 0 {
        format!("{label}, {unread} unread")
    } else {
        label.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nav_rows_name_the_unread_count() {
        assert_eq!(nav_row("Inbox", 3), "Inbox, 3 unread");
        assert_eq!(nav_row("Sent", 0), "Sent");
    }
}
