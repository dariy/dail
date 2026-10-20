//! Mailbox labels.

use app_core::{Mailbox, MailboxRole};

/// A mailbox's label: its role's name, else the last path segment.
pub fn mailbox_name(mailbox: &Mailbox) -> String {
    if let Some(role) = mailbox.role {
        return role_title(role).to_owned();
    }
    let delimiter = mailbox.delimiter.as_deref().unwrap_or("/");
    mailbox
        .path
        .rsplit(delimiter)
        .next()
        .unwrap_or(&mailbox.path)
        .to_owned()
}

/// The display name of a role.
pub const fn role_title(role: MailboxRole) -> &'static str {
    match role {
        MailboxRole::Inbox => "Inbox",
        MailboxRole::Sent => "Sent",
        MailboxRole::Drafts => "Drafts",
        MailboxRole::Trash => "Trash",
        MailboxRole::Archive => "Archive",
        MailboxRole::Junk => "Junk",
        MailboxRole::All => "All Mail",
        MailboxRole::Flagged => "Starred",
    }
}
