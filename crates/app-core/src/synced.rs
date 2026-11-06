//! Settings stored on the mail server (ADR 0015): the [`SyncedSettings`] document, sorting
//! [`Rule`]s, their validation and the pure merge of two copies.
//!
//! The document is public API like `config.toml` (AGENTS.md §5): versioned by `schema`, unknown
//! fields ignored, and frozen as a golden document in `crates/app-io/testdata/golden/settings/`.
//! Anyone with the password can write the folder that holds it, so every copy read from the
//! server is [validated](SyncedSettings::validate) before it is merged, and rule text is data,
//! never instructions.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{AccountId, RuleId};
use crate::state::SyncIssue;

/// The schema version this build writes. A copy with a higher version is still read: its unknown
/// fields are ignored.
pub const SETTINGS_SCHEMA: u32 = 1;

/// The largest document accepted from the server, in bytes of JSON.
pub const SETTINGS_MAX_BYTES: usize = 256 * 1024;

/// How long a deleted rule's tombstone is kept, so that a device that was offline does not bring
/// the rule back: 90 days in milliseconds.
pub const TOMBSTONE_TTL_MS: i64 = 90 * 24 * 60 * 60 * 1000;

/// Upper bounds of a valid document. They keep a hostile copy from costing time or memory.
pub const MAX_RULES: usize = 500;
/// The deepest nesting of `all` / `any` / `not` in one condition.
pub const MAX_CONDITION_DEPTH: usize = 8;
/// The longest name, pattern, folder path or label, in characters.
pub const MAX_TEXT_CHARS: usize = 512;
/// The most actions of one rule.
pub const MAX_ACTIONS: usize = 16;

/// A value with the time it was last set, for the field-by-field merge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Stamped<T> {
    /// Unix milliseconds of the change, from `Ctx::now_ms` of the device that made it.
    pub updated_ms: i64,
    /// The value.
    pub value: T,
}

/// The settings that follow the user across devices, stored in the account's `dail/Settings`
/// folder (ADR 0015). Window sizes, paths, per-device keymaps and secrets are never here.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct SyncedSettings {
    /// Version of this format; this build writes [`SETTINGS_SCHEMA`].
    pub schema: u32,
    /// Unix milliseconds of the newest change in the document.
    pub updated_ms: i64,
    /// The theme, if set on some device.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<Stamped<SyncedTheme>>,
    /// Shared preferences by setting name (`set_setting` keys), each with its own time.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub prefs: BTreeMap<String, Stamped<serde_json::Value>>,
    /// Sorting rules by ID, including the tombstones of deleted ones.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub rules: BTreeMap<RuleId, RuleEntry>,
}

/// The theme part of [`SyncedSettings`] (ADR 0008).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SyncedTheme {
    /// 0 = follow the system, 1 = light, 2 = dark (as `Settings::theme`).
    pub mode: u64,
    /// The built-in theme of the light mode, by name.
    pub light: String,
    /// The built-in theme of the dark mode, by name.
    pub dark: String,
}

/// One rule slot: the rule, or the tombstone of a deleted one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RuleEntry {
    /// Unix milliseconds of the last save or of the delete.
    pub updated_ms: i64,
    /// The rule; `None` once deleted (a tombstone, kept [`TOMBSTONE_TTL_MS`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<Rule>,
}

/// A sorting rule: when `when` matches a new message, the engine runs `then` as ordinary
/// commands with `Origin::Rule`. Rules cannot send, forward or reply (ADR 0015).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Rule {
    /// A name for people, e.g. `Newsletters to Reading`. Data, never instructions.
    pub name: String,
    /// Whether the rule runs. A disabled rule is kept but never matches.
    #[serde(default = "enabled_default")]
    pub enabled: bool,
    /// The condition over a message's envelope. On fetch, a rule without a `mailbox` test
    /// applies to Inbox mail only.
    pub when: Condition,
    /// What to do with a matching message, in order. At least one action.
    pub then: Vec<RuleAction>,
}

const fn enabled_default() -> bool {
    true
}

/// A condition over a message's envelope, tagged by `test`. Combine conditions with `all`,
/// `any` and `not`. Text tests (`from`, `to`, `cc`, `subject`, `list_id`) are
/// case-insensitive substring matches. The `mailbox` test compares a folder path exactly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "test", rename_all = "snake_case")]
pub enum Condition {
    /// Every one of `of` matches (an empty list always matches).
    All {
        /// The conditions.
        of: Vec<Condition>,
    },
    /// At least one of `of` matches (an empty list never matches).
    Any {
        /// The conditions.
        of: Vec<Condition>,
    },
    /// `of` does not match.
    Not {
        /// The negated condition.
        of: Box<Condition>,
    },
    /// The sender's address or name contains `contains`.
    From {
        /// The text to look for.
        contains: String,
    },
    /// A `To` recipient's address or name contains `contains`.
    To {
        /// The text to look for.
        contains: String,
    },
    /// A `Cc` recipient's address or name contains `contains`.
    Cc {
        /// The text to look for.
        contains: String,
    },
    /// The subject contains `contains`.
    Subject {
        /// The text to look for.
        contains: String,
    },
    /// The `List-Id` header contains `contains`.
    ListId {
        /// The text to look for.
        contains: String,
    },
    /// The message has at least one attachment.
    HasAttachment,
    /// The message is in the folder with this full path (e.g. `INBOX`).
    Mailbox {
        /// The folder's full path, compared exactly.
        path: String,
    },
}

/// What a rule does with a matching message, tagged by `action`. There is deliberately no
/// action that sends, forwards, replies or redirects mail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum RuleAction {
    /// Move the message to the folder with this full path.
    Move {
        /// The target folder's full path.
        to: String,
    },
    /// Copy the message into the folder with this path (a Gmail label). This version skips
    /// this action when the rule runs.
    AddLabel {
        /// The label, as a folder path.
        label: String,
    },
    /// Mark the message read.
    MarkRead,
    /// Star the message.
    Star,
    /// Archive the message.
    Archive,
}

/// Why a settings document from the server was rejected. It is ignored and the error shown.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error")]
pub enum SettingsError {
    /// The document is larger than [`SETTINGS_MAX_BYTES`].
    #[error("ERR020: the settings document has {bytes} bytes, more than the {max} allowed")]
    #[serde(rename = "error_020_settings_too_large")]
    TooLarge {
        /// Its size.
        bytes: usize,
        /// The limit.
        max: usize,
    },
    /// The document is not valid JSON of the schema.
    #[error("ERR021: the settings document is not valid: {reason}")]
    #[serde(rename = "error_021_settings_malformed")]
    Malformed {
        /// Parser diagnostic.
        reason: String,
    },
    /// The document parses but breaks a limit or rule of the format.
    #[error("ERR022: the settings document breaks a rule at `{at}`: {reason}")]
    #[serde(rename = "error_022_settings_invalid")]
    Invalid {
        /// Where, e.g. `rules.<id>.when`.
        at: String,
        /// What is wrong.
        reason: String,
    },
    /// Reading the settings folder failed (connection, login, IMAP), with a diagnostic that
    /// carries no code of its own.
    #[error("ERR023: the settings could not be read from the server: {reason}")]
    #[serde(rename = "error_023_settings_read_failed")]
    ReadFailed {
        /// The diagnostic.
        reason: String,
    },
    /// Storing the settings document failed, with a diagnostic that carries no code of its own.
    #[error("ERR024: the settings could not be stored on the server: {reason}")]
    #[serde(rename = "error_024_settings_write_failed")]
    WriteFailed {
        /// The diagnostic.
        reason: String,
    },
}

/// Whether `text` starts with an error code, `ERR<digits>: `.
pub fn has_error_code(text: &str) -> bool {
    text.strip_prefix("ERR").is_some_and(|rest| {
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        digits > 0 && rest[digits..].starts_with(": ")
    })
}

impl SettingsError {
    fn invalid(at: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Invalid {
            at: at.into(),
            reason: reason.into(),
        }
    }
}

impl SyncedSettings {
    /// An empty document of the current schema.
    pub const fn new() -> Self {
        Self {
            schema: SETTINGS_SCHEMA,
            updated_ms: 0,
            theme: None,
            prefs: BTreeMap::new(),
            rules: BTreeMap::new(),
        }
    }

    /// Reads a copy from the server: at most [`SETTINGS_MAX_BYTES`], valid JSON, and
    /// [valid](Self::validate). Unknown fields are ignored.
    pub fn from_json(bytes: &[u8]) -> Result<Self, SettingsError> {
        if bytes.len() > SETTINGS_MAX_BYTES {
            return Err(SettingsError::TooLarge {
                bytes: bytes.len(),
                max: SETTINGS_MAX_BYTES,
            });
        }
        let doc: Self = serde_json::from_slice(bytes).map_err(|e| SettingsError::Malformed {
            reason: e.to_string(),
        })?;
        doc.validate()?;
        Ok(doc)
    }

    /// The JSON written to the server, stamped with the current schema.
    pub fn to_json(&self) -> Result<Vec<u8>, SettingsError> {
        let mut doc = self.clone();
        doc.schema = doc.schema.max(SETTINGS_SCHEMA);
        let bytes = serde_json::to_vec_pretty(&doc).map_err(|e| SettingsError::Malformed {
            reason: e.to_string(),
        })?;
        if bytes.len() > SETTINGS_MAX_BYTES {
            return Err(SettingsError::TooLarge {
                bytes: bytes.len(),
                max: SETTINGS_MAX_BYTES,
            });
        }
        Ok(bytes)
    }

    /// The live rules (not tombstones), in ID order.
    pub fn live_rules(&self) -> impl Iterator<Item = (RuleId, &Rule)> {
        self.rules
            .iter()
            .filter_map(|(id, entry)| entry.rule.as_ref().map(|rule| (*id, rule)))
    }

    /// Checks the limits of the format. A copy from the server that fails is ignored.
    pub fn validate(&self) -> Result<(), SettingsError> {
        if self.schema == 0 {
            return Err(SettingsError::invalid("schema", "must be at least 1"));
        }
        if self.rules.len() > MAX_RULES {
            return Err(SettingsError::invalid(
                "rules",
                format!("more than {MAX_RULES} rules"),
            ));
        }
        if let Some(theme) = &self.theme {
            check_text("theme.light", &theme.value.light)?;
            check_text("theme.dark", &theme.value.dark)?;
            if theme.value.mode > 2 {
                return Err(SettingsError::invalid("theme.mode", "must be 0, 1 or 2"));
            }
        }
        for key in self.prefs.keys() {
            check_text("prefs", key)?;
        }
        for (id, entry) in &self.rules {
            if let Some(rule) = &entry.rule {
                rule.validate(&format!("rules.{id}"))?;
            }
        }
        Ok(())
    }

    /// Drops the tombstones older than [`TOMBSTONE_TTL_MS`] at `now_ms`.
    pub fn prune(&mut self, now_ms: i64) {
        self.rules.retain(|_, entry| {
            entry.rule.is_some() || now_ms.saturating_sub(entry.updated_ms) <= TOMBSTONE_TTL_MS
        });
    }

    /// Merges two copies field by field and rule by rule: the newer `updated_ms` wins, a
    /// tombstone wins a tie with a rule, and remaining ties go to the larger value, so the result
    /// does not depend on the order (commutative, associative and idempotent).
    #[must_use]
    pub fn merge(&self, other: &Self) -> Self {
        let mut prefs = self.prefs.clone();
        for (key, theirs) in &other.prefs {
            let pick = match prefs.get(key) {
                Some(ours) => newer(ours, theirs).clone(),
                None => theirs.clone(),
            };
            prefs.insert(key.clone(), pick);
        }
        let mut rules = self.rules.clone();
        for (id, theirs) in &other.rules {
            let pick = match rules.get(id) {
                Some(ours) => newer_rule(ours, theirs).clone(),
                None => theirs.clone(),
            };
            rules.insert(*id, pick);
        }
        let theme = match (&self.theme, &other.theme) {
            (Some(a), Some(b)) => Some(newer(a, b).clone()),
            (a, b) => a.clone().or_else(|| b.clone()),
        };
        Self {
            schema: self.schema.max(other.schema),
            updated_ms: self.updated_ms.max(other.updated_ms),
            theme,
            prefs,
            rules,
        }
    }
}

impl Rule {
    /// Checks the limits of one rule; `at` names it in the error.
    pub fn validate(&self, at: &str) -> Result<(), SettingsError> {
        if self.name.trim().is_empty() {
            return Err(SettingsError::invalid(format!("{at}.name"), "is empty"));
        }
        check_text(&format!("{at}.name"), &self.name)?;
        if self.then.is_empty() {
            return Err(SettingsError::invalid(
                format!("{at}.then"),
                "has no action",
            ));
        }
        if self.then.len() > MAX_ACTIONS {
            return Err(SettingsError::invalid(
                format!("{at}.then"),
                format!("more than {MAX_ACTIONS} actions"),
            ));
        }
        for action in &self.then {
            match action {
                RuleAction::Move { to: path } | RuleAction::AddLabel { label: path } => {
                    if path.is_empty() {
                        return Err(SettingsError::invalid(
                            format!("{at}.then"),
                            "a folder path is empty",
                        ));
                    }
                    check_text(&format!("{at}.then"), path)?;
                }
                RuleAction::MarkRead | RuleAction::Star | RuleAction::Archive => {}
            }
        }
        self.when.validate(&format!("{at}.when"), 1)
    }
}

impl Condition {
    fn validate(&self, at: &str, depth: usize) -> Result<(), SettingsError> {
        if depth > MAX_CONDITION_DEPTH {
            return Err(SettingsError::invalid(
                at,
                format!("nested deeper than {MAX_CONDITION_DEPTH}"),
            ));
        }
        match self {
            Self::All { of } | Self::Any { of } => {
                if of.len() > MAX_RULES {
                    return Err(SettingsError::invalid(at, "too many conditions"));
                }
                of.iter().try_for_each(|c| c.validate(at, depth + 1))
            }
            Self::Not { of } => of.validate(at, depth + 1),
            Self::From { contains }
            | Self::To { contains }
            | Self::Cc { contains }
            | Self::Subject { contains }
            | Self::ListId { contains } => {
                if contains.is_empty() {
                    return Err(SettingsError::invalid(at, "a text test is empty"));
                }
                check_text(at, contains)
            }
            Self::Mailbox { path } => check_text(at, path),
            Self::HasAttachment => Ok(()),
        }
    }
}

fn check_text(at: &str, text: &str) -> Result<(), SettingsError> {
    if text.chars().count() > MAX_TEXT_CHARS {
        return Err(SettingsError::invalid(
            at,
            format!("longer than {MAX_TEXT_CHARS} characters"),
        ));
    }
    Ok(())
}

/// Orders two values by their canonical JSON, the tie-break of equal times.
fn json_key<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

fn newer<'a, T: Serialize>(a: &'a Stamped<T>, b: &'a Stamped<T>) -> &'a Stamped<T> {
    match a.updated_ms.cmp(&b.updated_ms) {
        std::cmp::Ordering::Greater => a,
        std::cmp::Ordering::Less => b,
        std::cmp::Ordering::Equal => {
            if json_key(&a.value) >= json_key(&b.value) {
                a
            } else {
                b
            }
        }
    }
}

fn newer_rule<'a>(a: &'a RuleEntry, b: &'a RuleEntry) -> &'a RuleEntry {
    match a.updated_ms.cmp(&b.updated_ms) {
        std::cmp::Ordering::Greater => a,
        std::cmp::Ordering::Less => b,
        std::cmp::Ordering::Equal => match (&a.rule, &b.rule) {
            (None, _) => a,
            (_, None) => b,
            (Some(x), Some(y)) => {
                if json_key(x) >= json_key(y) {
                    a
                } else {
                    b
                }
            }
        },
    }
}

/// The top-level folder that holds dail's own data on the server (ADR 0015).
pub const SETTINGS_PARENT: &str = "dail";

/// The settings folder's name below [`SETTINGS_PARENT`].
pub const SETTINGS_LEAF: &str = "Settings";

/// The IMAP path of the settings folder for an account whose hierarchy delimiter is
/// `separator`: `dail/Settings` (or `dail.Settings`).
pub fn settings_folder(separator: &str) -> String {
    format!("{SETTINGS_PARENT}{separator}{SETTINGS_LEAF}")
}

/// The server copy of the settings document that was read last: the message with the highest
/// UID in the settings folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SettingsCopy {
    /// The folder's UIDVALIDITY.
    pub uid_validity: u32,
    /// The message's UID.
    pub uid: u32,
}

/// The settings that follow the user, as this device knows them (ADR 0015): the merged
/// document and where its sync with the settings account stands.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct SyncedState {
    /// The merged document: local changes and every copy pulled so far.
    pub settings: SyncedSettings,
    /// The account the last pull read from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<AccountId>,
    /// The server copy read last (`None`: none yet, or the folder is empty or missing).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copy: Option<SettingsCopy>,
    /// When the last pull finished, in Unix milliseconds. `push_settings` waits for one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pulled_ms: Option<i64>,
    /// When the last push was stored on the server, in Unix milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pushed_ms: Option<i64>,
    /// A pull is in flight.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub pulling: bool,
    /// The account the pull in flight reads from: a late answer from another account (the
    /// settings account changed or was removed meanwhile) is ignored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pulling_from: Option<AccountId>,
    /// A push is in flight.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub pushing: bool,
    /// The document has local changes (`save_rule`, `delete_rule`, `enable_rule`) that no push
    /// has taken yet. A push clears it when it starts; a failed push sets it again.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub dirty: bool,
    /// The last failed pull or push, or the reason a server copy was ignored (too large,
    /// malformed or invalid), until a pull or push succeeds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<SyncIssue>,
}

/// The part of [`SyncedState`] kept across restarts in the profile (`state.redb`, table
/// `synced`): the merged document and whether it has local changes no push has taken. A profile
/// without the row starts with an empty document.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct LocalSettings {
    /// The merged document.
    pub settings: SyncedSettings,
    /// Local changes no push has taken yet.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub dirty: bool,
}

impl LocalSettings {
    /// What `state` keeps of the synced settings.
    pub fn of(state: &SyncedState) -> Self {
        Self {
            settings: state.settings.clone(),
            dirty: state.dirty,
        }
    }

    /// Restores the document and the flag into `state`.
    pub fn apply_to(&self, state: &mut crate::state::AppState) {
        state.synced.settings = self.settings.clone();
        state.synced.dirty = self.dirty;
    }
}

impl SyncedState {
    /// Nothing pulled, pushed or changed yet.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}
