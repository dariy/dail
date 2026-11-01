//! `CoreError`: structured, serializable errors returned by the engine.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Why the engine rejected a command or event. Serialized as `{"error":"error_<code>_<slug>", …}` so
/// agents get machine-readable failures; `Display` starts with `ERR<code>: ` (ADR 0013). A rejected input never changes state.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error")]
pub enum CoreError {
    /// The JSON is not a known command or its arguments do not match the command's schema.
    #[error("ERR001: invalid command: {reason}")]
    #[serde(rename = "error_001_invalid_command")]
    InvalidCommand {
        /// Parser diagnostic.
        reason: String,
    },
    /// An argument is well-formed JSON but not an acceptable value.
    #[error("ERR002: invalid argument `{field}`: {reason}")]
    #[serde(rename = "error_002_invalid_argument")]
    InvalidArgument {
        /// Name of the offending argument.
        field: String,
        /// Why it was rejected.
        reason: String,
    },
    /// A replay log line is not a valid record.
    #[error("ERR003: invalid replay record on line {line}: {reason}")]
    #[serde(rename = "error_003_invalid_replay")]
    InvalidReplay {
        /// 1-based line number in the log.
        line: usize,
        /// Parser diagnostic.
        reason: String,
    },
    /// The referenced entity does not exist (any more).
    #[error("ERR004: {entity} {id} not found")]
    #[serde(rename = "error_004_not_found")]
    NotFound {
        /// Entity type, e.g. `account`.
        entity: String,
        /// The ID that was looked up.
        id: String,
    },
    /// An entity with the same unique key exists already.
    #[error("ERR005: {entity} {key} already exists")]
    #[serde(rename = "error_005_already_exists")]
    AlreadyExists {
        /// Entity type, e.g. `account`.
        entity: String,
        /// The conflicting key, e.g. the email address.
        key: String,
    },
    /// A `Destructive` command was sent without `confirm: true`.
    #[error("ERR006: {command} is irreversible; repeat it with `confirm: true`")]
    #[serde(rename = "error_006_confirmation_required")]
    ConfirmationRequired {
        /// Name of the command.
        command: String,
    },
    /// The account has no mailbox for this purpose (e.g. no Trash folder), so the message
    /// cannot be moved there. Use `move_threads` with an explicit mailbox instead.
    #[error("ERR007: account {account} has no {role:?} mailbox")]
    #[serde(rename = "error_007_no_mailbox_for_role")]
    NoMailboxForRole {
        /// The account.
        account: String,
        /// The missing role.
        role: crate::mailbox::MailboxRole,
    },
    /// Only some front-ends may run this command (e.g. agents can't approve their own sends).
    #[error(
        "ERR008: {command} can't be run from {origin}; the user has to do it in the GUI or the CLI"
    )]
    #[serde(rename = "error_008_origin_not_allowed")]
    OriginNotAllowed {
        /// Name of the command.
        command: String,
        /// The front-end that tried.
        origin: String,
    },
    /// The draft can't be sent (or changed) in its current state.
    #[error("ERR009: draft {draft} can't be sent: {reason}")]
    #[serde(rename = "error_009_not_sendable")]
    NotSendable {
        /// The draft.
        draft: String,
        /// Why, and what to do about it.
        reason: String,
    },
    /// `undo` was requested but the history is empty.
    #[error("ERR010: nothing to undo")]
    #[serde(rename = "error_010_nothing_to_undo")]
    NothingToUndo,
    /// `redo` was requested but nothing has been undone since the last change.
    #[error("ERR011: nothing to redo")]
    #[serde(rename = "error_011_nothing_to_redo")]
    NothingToRedo,
}

impl CoreError {
    /// The stable code, as the user quotes it. The same in every language; never reused.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidCommand { .. } => "ERR001",
            Self::InvalidArgument { .. } => "ERR002",
            Self::InvalidReplay { .. } => "ERR003",
            Self::NotFound { .. } => "ERR004",
            Self::AlreadyExists { .. } => "ERR005",
            Self::ConfirmationRequired { .. } => "ERR006",
            Self::NoMailboxForRole { .. } => "ERR007",
            Self::OriginNotAllowed { .. } => "ERR008",
            Self::NotSendable { .. } => "ERR009",
            Self::NothingToUndo => "ERR010",
            Self::NothingToRedo => "ERR011",
        }
    }
}
