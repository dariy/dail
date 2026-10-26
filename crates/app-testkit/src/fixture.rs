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

/// The generated built-in: `basic` with [`LARGE_MESSAGES`] more messages in its inbox.
const LARGE: &str = "large";

/// The size of the `large` fixture's inbox, for scale measurements (ROADMAP Wave 5 I3).
pub const LARGE_MESSAGES: u32 = 20_000;

impl Fixture {
    /// Names of the built-in fixtures.
    pub fn builtin_names() -> Vec<&'static str> {
        let mut names: Vec<_> = BUILTIN.iter().map(|(name, _)| *name).collect();
        names.push(LARGE);
        names
    }

    /// Loads a built-in fixture by name.
    pub fn builtin(name: &str) -> Result<Self, TestkitError> {
        if name == LARGE {
            return Self::large(LARGE_MESSAGES);
        }
        let (_, json) = BUILTIN
            .iter()
            .find(|(builtin, _)| *builtin == name)
            .ok_or_else(|| TestkitError::UnknownFixture {
                name: name.to_owned(),
                known: Self::builtin_names().join(", "),
            })?;
        Self::from_json(json)
    }

    /// `basic` with `extra` generated messages in its first account's inbox: one sender in 50, a
    /// thread every 4 messages, a few lines of text each, one in 3 read, dated a minute apart
    /// before `basic`'s own. Deterministic, so that measurements compare.
    pub fn large(extra: u32) -> Result<Self, TestkitError> {
        let mut fixture = Self::builtin("basic")?;
        let inbox = fixture
            .accounts
            .first_mut()
            .and_then(|account| account.mailboxes.iter_mut().find(|m| m.path == "INBOX"))
            .ok_or_else(|| TestkitError::Invalid("basic has no INBOX".to_owned()))?;
        let own = std::mem::take(&mut inbox.messages);
        // 2023-11-01 00:00 UTC, before basic's messages of 14 Nov.
        const START: i64 = 1_698_796_800;
        for n in 0..extra {
            let thread = n / 4;
            let reply = n % 4 != 0;
            let date = START - i64::from(extra - n) * 60;
            let raw = format!(
                "Message-ID: <m{n}@large.example>\r\n{refs}From: Sender {sender} <s{sender}@large.example>\r\n\
                 To: alice@example.com\r\nSubject: {re}Report {thread}\r\nDate: {date}\r\n\r\n\
                 Line one of message {n} about project {project}.\r\nLine two with words to index: \
                 budget, schedule, review, {project}.\r\nRegards,\r\nSender {sender}\r\n",
                refs = if reply {
                    format!("In-Reply-To: <m{}@large.example>\r\n", n - 1)
                } else {
                    String::new()
                },
                sender = n % 50,
                re = if reply { "Re: " } else { "" },
                project = n % 97,
                date = rfc2822(date),
            );
            let flags = if n % 3 == 0 {
                vec!["\\Seen".to_owned()]
            } else {
                Vec::new()
            };
            inbox.append(flags, raw);
        }
        for message in own {
            inbox.append(message.flags, message.raw);
        }
        fixture.validate()?;
        Ok(fixture)
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

/// `Tue, 14 Nov 2023 09:41:00 +0000` for `secs` since the epoch (UTC).
fn rfc2822(secs: i64) -> String {
    const DAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let weekday = DAYS
        .get(usize::try_from(days.rem_euclid(7)).unwrap_or(0))
        .unwrap_or(&"Mon");
    let month_name = MONTHS
        .get(usize::try_from(month - 1).unwrap_or(0))
        .unwrap_or(&"Jan");
    format!(
        "{weekday}, {day:02} {month_name} {year} {:02}:{:02}:{:02} +0000",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    )
}
