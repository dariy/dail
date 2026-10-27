//! Mailboxes (IMAP folders) and their roles (`ARCHITECTURE.md §4.2, §5.2`).
//!
//! The server reports mailboxes as [`RemoteMailbox`]es. [`assign_roles`] gives each one a
//! [`MailboxRole`] from, in order of preference, the SPECIAL-USE attribute, the provider preset's
//! folder names and name heuristics.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::account::ProviderPreset;
use crate::ids::{AccountId, MailboxId};

/// A mailbox of one account, as cached locally.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Mailbox {
    /// Mailbox ID.
    pub id: MailboxId,
    /// The account the mailbox belongs to.
    pub account: AccountId,
    /// Full IMAP path, e.g. `INBOX` or `[Gmail]/Sent Mail`.
    pub path: String,
    /// Hierarchy delimiter reported by the server, e.g. `/` or `.`.
    pub delimiter: Option<String>,
    /// What the mailbox is used for, if known.
    pub role: Option<MailboxRole>,
    /// UIDVALIDITY of the cached messages (`None` until the first fetch).
    pub uid_validity: Option<u32>,
    /// The UID the next new message will get, as of the last fetch.
    pub uid_next: Option<u32>,
    /// Unread messages on the server, as of the last fetch.
    pub unread: u32,
    /// Messages on the server, as of the last fetch.
    pub total: u32,
    /// The server's HIGHESTMODSEQ (CONDSTORE) as of the last flag resync: the next resync asks
    /// only for changes since then. `None` without CONDSTORE (or before the first resync).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highest_modseq: Option<u64>,
    /// How many of the newest messages to keep cached (`set_sync_window`): `Some(0)` means the
    /// whole mailbox, `None` the `initial_window` setting. Older ones are fetched in the
    /// background.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_window: Option<u32>,
    /// Whether the oldest message on the server is cached (nothing older to fetch).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub history_complete: bool,
    /// The address new drafts from this mailbox are sent from (`set_mailbox_identity`), if not
    /// the account's own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<String>,
    /// When its envelopes were last fetched, in Unix ms (`open_mailbox` fetches again when
    /// this is older than `stale_secs`; UX-6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_fetch_ms: Option<i64>,
    /// `Some(false)` once the server refused to keep keywords here (`PERMANENTFLAGS` without
    /// `\*`): pins stay on the device and the server's flags never clear them (MISC-4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_keywords: Option<bool>,
}

impl Mailbox {
    /// The hierarchy delimiter, `/` when the server named none.
    pub fn separator(&self) -> &str {
        self.delimiter
            .as_deref()
            .filter(|d| !d.is_empty())
            .unwrap_or("/")
    }

    /// The path of the folder above this one (`Work/Clients` for `Work/Clients/Acme`), if any.
    pub fn parent_path(&self) -> Option<&str> {
        self.path
            .rsplit_once(self.separator())
            .map(|(parent, _)| parent)
            .filter(|parent| !parent.is_empty())
    }

    /// How many folders are above this one (`0` for a top-level folder).
    pub fn depth(&self) -> usize {
        self.path
            .split(self.separator())
            .filter(|segment| !segment.is_empty())
            .count()
            .saturating_sub(1)
    }

    /// The key of this folder in the folder tree.
    pub fn key(&self) -> FolderKey {
        FolderKey {
            account: self.account,
            path: self.path.clone(),
        }
    }
}

/// A node of an account's folder tree: a mailbox, or a parent folder that is not cached
/// (`\Noselect`). An empty `path` stands for the whole account.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub struct FolderKey {
    /// The account.
    pub account: AccountId,
    /// The folder's full path; empty for the account itself.
    #[serde(default)]
    pub path: String,
}

/// The purpose of a mailbox.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum MailboxRole {
    /// Incoming mail.
    Inbox,
    /// Copies of sent mail.
    Sent,
    /// Unsent drafts.
    Drafts,
    /// Deleted mail.
    Trash,
    /// Archived mail.
    Archive,
    /// Spam.
    Junk,
    /// Every message of the account (Gmail "All Mail").
    All,
    /// Starred / flagged messages (a virtual mailbox).
    Flagged,
}

impl MailboxRole {
    /// Every role, in display order.
    pub const ALL: &'static [Self] = &[
        Self::Inbox,
        Self::Sent,
        Self::Drafts,
        Self::Archive,
        Self::All,
        Self::Flagged,
        Self::Junk,
        Self::Trash,
    ];

    /// The role's `snake_case` name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Inbox => "inbox",
            Self::Sent => "sent",
            Self::Drafts => "drafts",
            Self::Trash => "trash",
            Self::Archive => "archive",
            Self::Junk => "junk",
            Self::All => "all",
            Self::Flagged => "flagged",
        }
    }

    /// Parses a role name (`inbox`, `sent`, …) or an RFC 6154 SPECIAL-USE attribute
    /// (`\Sent`, `\Junk`, …), case-insensitively.
    pub fn from_special_use(name: &str) -> Option<Self> {
        let name = name.trim_start_matches('\\');
        Self::ALL
            .iter()
            .copied()
            .find(|role| role.as_str().eq_ignore_ascii_case(name))
    }

    /// Position in display order (Inbox first, Trash last).
    pub fn rank(self) -> usize {
        Self::ALL
            .iter()
            .position(|role| *role == self)
            .unwrap_or(Self::ALL.len())
    }
}

/// A mailbox as listed by the server (`LIST`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RemoteMailbox {
    /// Full IMAP path.
    pub path: String,
    /// Hierarchy delimiter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delimiter: Option<String>,
    /// RFC 6154 SPECIAL-USE attribute, if the server advertises one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub special_use: Option<MailboxRole>,
    /// `false` for `\Noselect` / `\NonExistent` entries, which hold no messages.
    pub selectable: bool,
}

/// Folder names of a provider preset, with the role each one has.
const GMAIL_ROLES: &[(&str, MailboxRole)] = &[
    ("[Gmail]/Sent Mail", MailboxRole::Sent),
    ("[Gmail]/Drafts", MailboxRole::Drafts),
    ("[Gmail]/Trash", MailboxRole::Trash),
    ("[Gmail]/Bin", MailboxRole::Trash),
    ("[Gmail]/Spam", MailboxRole::Junk),
    ("[Gmail]/All Mail", MailboxRole::All),
    ("[Gmail]/Starred", MailboxRole::Flagged),
    ("[Google Mail]/Sent Mail", MailboxRole::Sent),
    ("[Google Mail]/Drafts", MailboxRole::Drafts),
    ("[Google Mail]/Trash", MailboxRole::Trash),
    ("[Google Mail]/Bin", MailboxRole::Trash),
    ("[Google Mail]/Spam", MailboxRole::Junk),
    ("[Google Mail]/All Mail", MailboxRole::All),
    ("[Google Mail]/Starred", MailboxRole::Flagged),
];

const HOSTINGER_ROLES: &[(&str, MailboxRole)] = &[
    ("INBOX.Sent", MailboxRole::Sent),
    ("INBOX.Drafts", MailboxRole::Drafts),
    ("INBOX.Trash", MailboxRole::Trash),
    ("INBOX.Junk", MailboxRole::Junk),
    ("INBOX.Spam", MailboxRole::Junk),
    ("INBOX.Archive", MailboxRole::Archive),
    ("Sent", MailboxRole::Sent),
    ("Drafts", MailboxRole::Drafts),
    ("Trash", MailboxRole::Trash),
    ("Junk", MailboxRole::Junk),
    ("Spam", MailboxRole::Junk),
    ("Archive", MailboxRole::Archive),
];

impl ProviderPreset {
    /// Folder paths whose role this preset knows (`ARCHITECTURE.md §5.3`).
    pub const fn folder_roles(self) -> &'static [(&'static str, MailboxRole)] {
        match self {
            Self::Gmail => GMAIL_ROLES,
            Self::Hostinger => HOSTINGER_ROLES,
            Self::Custom => &[],
        }
    }
}

/// Common folder names (last path segment, lower case) and their roles.
const NAME_HEURISTICS: &[(&str, MailboxRole)] = &[
    ("sent", MailboxRole::Sent),
    ("sent items", MailboxRole::Sent),
    ("sent messages", MailboxRole::Sent),
    ("sent mail", MailboxRole::Sent),
    ("drafts", MailboxRole::Drafts),
    ("draft", MailboxRole::Drafts),
    ("trash", MailboxRole::Trash),
    ("deleted items", MailboxRole::Trash),
    ("deleted messages", MailboxRole::Trash),
    ("bin", MailboxRole::Trash),
    ("junk", MailboxRole::Junk),
    ("junk e-mail", MailboxRole::Junk),
    ("junk email", MailboxRole::Junk),
    ("spam", MailboxRole::Junk),
    ("bulk mail", MailboxRole::Junk),
    ("archive", MailboxRole::Archive),
    ("archives", MailboxRole::Archive),
    ("all mail", MailboxRole::All),
    ("starred", MailboxRole::Flagged),
    ("flagged", MailboxRole::Flagged),
];

/// One source of mailbox roles, tried for every mailbox before the next source.
type RoleTier<'a> = dyn Fn(&RemoteMailbox) -> Option<MailboxRole> + 'a;

/// The role of every mailbox in `listed`, in the same order.
///
/// `INBOX` (any case) is always the inbox. Then SPECIAL-USE attributes win, then the preset's
/// folder names, then name heuristics. Each role is given to at most one mailbox; ties within a
/// tier go to the mailbox listed first.
pub fn assign_roles(preset: ProviderPreset, listed: &[RemoteMailbox]) -> Vec<Option<MailboxRole>> {
    let mut roles: Vec<Option<MailboxRole>> = vec![None; listed.len()];
    let tiers: [&RoleTier<'_>; 4] = [
        &|mailbox| {
            mailbox
                .path
                .eq_ignore_ascii_case("INBOX")
                .then_some(MailboxRole::Inbox)
        },
        &|mailbox| mailbox.special_use,
        &|mailbox| {
            preset
                .folder_roles()
                .iter()
                .find(|(path, _)| *path == mailbox.path)
                .map(|(_, role)| *role)
        },
        &|mailbox| {
            let leaf = leaf_name(mailbox).to_lowercase();
            NAME_HEURISTICS
                .iter()
                .find(|(name, _)| *name == leaf)
                .map(|(_, role)| *role)
        },
    ];
    for tier in tiers {
        for (index, mailbox) in listed.iter().enumerate() {
            if roles[index].is_some() || !mailbox.selectable {
                continue;
            }
            if let Some(role) = tier(mailbox)
                && !roles.contains(&Some(role))
            {
                roles[index] = Some(role);
            }
        }
    }
    roles
}

/// The last segment of a mailbox path (`INBOX.Sent` → `Sent` with delimiter `.`).
fn leaf_name(mailbox: &RemoteMailbox) -> &str {
    match mailbox.delimiter.as_deref() {
        Some(delimiter) if !delimiter.is_empty() => mailbox
            .path
            .rsplit(delimiter)
            .next()
            .unwrap_or(&mailbox.path),
        _ => &mailbox.path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote(path: &str, special_use: Option<MailboxRole>) -> RemoteMailbox {
        RemoteMailbox {
            path: path.to_owned(),
            delimiter: Some("/".to_owned()),
            special_use,
            selectable: true,
        }
    }

    #[test]
    fn special_use_wins_over_preset_and_heuristics() {
        let listed = [
            remote("Sent", None),
            remote("Gesendet", Some(MailboxRole::Sent)),
            remote("inbox", None),
        ];
        let roles = assign_roles(ProviderPreset::Custom, &listed);
        assert_eq!(
            roles,
            [None, Some(MailboxRole::Sent), Some(MailboxRole::Inbox)]
        );
    }

    #[test]
    fn preset_mapping_beats_heuristics() {
        let listed = [
            remote("Sent Items", None),
            remote("[Gmail]/Sent Mail", None),
            remote("[Gmail]/All Mail", None),
            remote("[Gmail]", None),
        ];
        let roles = assign_roles(ProviderPreset::Gmail, &listed);
        assert_eq!(
            roles,
            [None, Some(MailboxRole::Sent), Some(MailboxRole::All), None]
        );
    }

    #[test]
    fn heuristics_use_the_leaf_name_and_skip_unselectable_entries() {
        let mut parent = remote("Archive", None);
        parent.selectable = false;
        let mut dotted = remote("INBOX.Deleted Items", None);
        dotted.delimiter = Some(".".to_owned());
        let listed = [
            parent,
            dotted,
            remote("Work/Archive", None),
            remote("Spam", None),
        ];
        let roles = assign_roles(ProviderPreset::Custom, &listed);
        assert_eq!(
            roles,
            [
                None,
                Some(MailboxRole::Trash),
                Some(MailboxRole::Archive),
                Some(MailboxRole::Junk)
            ]
        );
    }

    #[test]
    fn special_use_names_parse() {
        assert_eq!(
            MailboxRole::from_special_use("\\Junk"),
            Some(MailboxRole::Junk)
        );
        assert_eq!(
            MailboxRole::from_special_use("archive"),
            Some(MailboxRole::Archive)
        );
        assert_eq!(MailboxRole::from_special_use("\\Important"), None);
        assert!(MailboxRole::Inbox.rank() < MailboxRole::Trash.rank());
    }
}
