//! `CoreError`: structured, serializable errors returned by the engine.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Why the engine rejected a command or event. Serialized as `{"error":"<variant>", …}` so agents
/// get machine-readable failures. A rejected input never changes state.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum CoreError {
    /// The JSON is not a known command or its arguments do not match the command's schema.
    #[error("invalid command: {reason}")]
    InvalidCommand {
        /// Parser diagnostic.
        reason: String,
    },
    /// An argument is well-formed JSON but not an acceptable value.
    #[error("invalid argument `{field}`: {reason}")]
    InvalidArgument {
        /// Name of the offending argument.
        field: String,
        /// Why it was rejected.
        reason: String,
    },
    /// A replay log line is not a valid record.
    #[error("invalid replay record on line {line}: {reason}")]
    InvalidReplay {
        /// 1-based line number in the log.
        line: usize,
        /// Parser diagnostic.
        reason: String,
    },
    /// The referenced entity does not exist (any more).
    #[error("{entity} {id} not found")]
    NotFound {
        /// Entity type, e.g. `account`.
        entity: String,
        /// The ID that was looked up.
        id: String,
    },
    /// An entity with the same unique key exists already.
    #[error("{entity} {key} already exists")]
    AlreadyExists {
        /// Entity type, e.g. `account`.
        entity: String,
        /// The conflicting key, e.g. the email address.
        key: String,
    },
    /// A `Destructive` command was sent without `confirm: true`.
    #[error("{command} is irreversible; repeat it with `confirm: true`")]
    ConfirmationRequired {
        /// Name of the command.
        command: String,
    },
    /// The account has no mailbox for this purpose (e.g. no Trash folder), so the message
    /// cannot be moved there. Use `move_threads` with an explicit mailbox instead.
    #[error("account {account} has no {role:?} mailbox")]
    NoMailboxForRole {
        /// The account.
        account: String,
        /// The missing role.
        role: crate::mailbox::MailboxRole,
    },
    /// Only some front-ends may run this command (e.g. agents can't approve their own sends).
    #[error("{command} can't be run from {origin}; the user has to do it in the GUI or the CLI")]
    OriginNotAllowed {
        /// Name of the command.
        command: String,
        /// The front-end that tried.
        origin: String,
    },
    /// The draft can't be sent (or changed) in its current state.
    #[error("draft {draft} can't be sent: {reason}")]
    NotSendable {
        /// The draft.
        draft: String,
        /// Why, and what to do about it.
        reason: String,
    },
    /// `undo` was requested but the history is empty.
    #[error("nothing to undo")]
    NothingToUndo,
    /// `redo` was requested but nothing has been undone since the last change.
    #[error("nothing to redo")]
    NothingToRedo,
}
