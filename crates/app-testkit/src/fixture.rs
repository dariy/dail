//! Fixtures: the content of a fake mail server, loaded from JSON.
//!
//! Built-in fixtures live in `crates/app-testkit/fixtures/<name>.json` and are compiled in, so
//! `dail batch --backend memory --fixture basic` works from any directory.

use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Errors of the test kit.
#[derive(Debug, thiserror::Error)]
pub enum TestkitError {
    /// No built-in fixture has this name.
    #[error("unknown fixture `{name}` (built-in: {known})")]
    UnknownFixture {
        /// Requested name.
        name: String,
        /// Comma-separated built-in names.
        known: String,
    },
    /// The fixture file could not be read.
    #[error("cannot read fixture {path}: {source}")]
    Read {
        /// File path.
        path: String,
        /// Underlying error.
        source: std::io::Error,
    },
    /// The fixture is not valid JSON of the expected shape.
    #[error("invalid fixture JSON: {0}")]
    Parse(#[from] serde_json::Error),
    /// The fixture is well-formed but inconsistent.
    #[error("invalid fixture: {0}")]
    Invalid(String),
    /// The session over the fixture failed (`synced`).
    #[error("session: {0}")]
    Session(String),
}

/// The content of a fake server: accounts, mailboxes and raw messages.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fixture {
    /// Accounts on the fake server.
    #[serde(default)]
    pub accounts: Vec<FakeAccount>,
}

/// One account on the fake server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FakeAccount {
    /// Login / address.
    pub email: String,
    /// Password the fake IMAP/SMTP servers accept. `None` accepts any password.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    /// The account's mailboxes.
    #[serde(default)]
    pub mailboxes: Vec<FakeMailbox>,
    /// The mailbox the fake SMTP server files every sent message in itself (as Gmail does with
    /// `[Gmail]/Sent Mail`); `None`: the client must store the copy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smtp_saves_to: Option<String>,
}

/// One IMAP mailbox.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FakeMailbox {
    /// Full IMAP path, e.g. `INBOX` or `[Gmail]/All Mail`.
    pub path: String,
    /// SPECIAL-USE role without the backslash (`inbox`, `sent`, `archive`, …), if advertised.
    #[serde(default)]
    pub special_use: Option<String>,
    /// `false` for a `\Noselect` folder that only holds other folders.
    #[serde(default = "selectable_default")]
    pub selectable: bool,
    /// UIDVALIDITY.
    pub uid_validity: u32,
    /// Lowest UID a new message may get (UIDs are never reused, even after an expunge).
    /// Defaults to one more than the highest UID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uid_next: Option<u32>,
    /// Messages, in UID order.
    #[serde(default)]
    pub messages: Vec<FakeMessage>,
}

impl FakeMailbox {
    /// An empty, selectable mailbox without a SPECIAL-USE role.
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            special_use: None,
            selectable: true,
            uid_validity: 1,
            uid_next: None,
            messages: Vec::new(),
        }
    }

    /// The UID the next new message gets.
    pub fn next_uid(&self) -> u32 {
        let after_last = self.messages.last().map_or(1, |m| m.uid.saturating_add(1));
        self.uid_next
            .map_or(after_last, |next| next.max(after_last))
    }

    /// Appends a message with the next UID; returns that UID.
    pub fn append(&mut self, flags: Vec<String>, raw: String) -> u32 {
        let uid = self.next_uid();
        self.uid_next = Some(uid.saturating_add(1));
        self.messages.push(FakeMessage { uid, flags, raw });
        uid
    }
}

/// One message on the fake server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FakeMessage {
    /// IMAP UID, unique within its mailbox.
    pub uid: u32,
    /// IMAP flags, e.g. `\Seen`.
    #[serde(default)]
    pub flags: Vec<String>,
    /// Raw RFC 822 message.
    pub raw: String,
}

const fn selectable_default() -> bool {
    true
}

const BUILTIN: &[(&str, &str)] = &[("basic", include_str!("../fixtures/basic.json"))];

impl Fixture {
    /// Names of the built-in fixtures.
    pub fn builtin_names() -> Vec<&'static str> {
        BUILTIN.iter().map(|(name, _)| *name).collect()
    }

    /// Loads a built-in fixture by name.
    pub fn builtin(name: &str) -> Result<Self, TestkitError> {
        let (_, json) = BUILTIN
            .iter()
            .find(|(builtin, _)| *builtin == name)
            .ok_or_else(|| TestkitError::UnknownFixture {
                name: name.to_owned(),
                known: Self::builtin_names().join(", "),
            })?;
        Self::from_json(json)
    }

    /// Loads a fixture from a JSON file.
    pub fn from_path(path: &Path) -> Result<Self, TestkitError> {
        let json = std::fs::read_to_string(path).map_err(|source| TestkitError::Read {
            path: path.display().to_string(),
            source,
        })?;
        Self::from_json(&json)
    }

    /// Parses and validates a fixture.
    pub fn from_json(json: &str) -> Result<Self, TestkitError> {
        let fixture: Self = serde_json::from_str(json)?;
        fixture.validate()?;
        Ok(fixture)
    }

    /// Checks that emails, mailbox paths and UIDs are unique and UIDs are ascending.
    pub fn validate(&self) -> Result<(), TestkitError> {
        let mut emails = BTreeSet::new();
        for account in &self.accounts {
            if !emails.insert(account.email.as_str()) {
                return Err(TestkitError::Invalid(format!(
                    "duplicate account {}",
                    account.email
                )));
            }
            let mut paths = BTreeSet::new();
            for mailbox in &account.mailboxes {
                if !paths.insert(mailbox.path.as_str()) {
                    return Err(TestkitError::Invalid(format!(
                        "duplicate mailbox {} in {}",
                        mailbox.path, account.email
                    )));
                }
                let ascending = mailbox
                    .messages
                    .windows(2)
                    .all(|pair| matches!(pair, [a, b] if a.uid < b.uid));
                if !ascending {
                    return Err(TestkitError::Invalid(format!(
                        "UIDs in {} of {} must be unique and ascending",
                        mailbox.path, account.email
                    )));
                }
            }
        }
        Ok(())
    }

    /// Finds an account by email (case-insensitive).
    pub fn account(&self, email: &str) -> Option<&FakeAccount> {
        self.accounts
            .iter()
            .find(|account| account.email.eq_ignore_ascii_case(email))
    }

    /// Finds a mailbox by account email and path.
    pub fn mailbox(&self, email: &str, path: &str) -> Option<&FakeMailbox> {
        self.accounts
            .iter()
            .find(|account| account.email == email)?
            .mailboxes
            .iter()
            .find(|mailbox| mailbox.path == path)
    }
}
