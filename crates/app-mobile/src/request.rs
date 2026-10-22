//! What the presenter submits and what comes back.

use app_core::{Command, CommandName, CommandReceipt, CoreError, ThreadId};

/// Why the presenter submitted a command; decides what happens with the result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Purpose {
    /// A user action: rejections and undo labels show in the snackbar.
    User,
    /// `get_thread` with bodies for the reader (`app_front::body`).
    Bodies(ThreadId),
}

/// A command and why it was submitted.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    /// Why it was submitted.
    pub purpose: Purpose,
    /// The command.
    pub cmd: Command,
}

impl Request {
    /// A user action.
    pub const fn user(cmd: Command) -> Self {
        Self {
            purpose: Purpose::User,
            cmd,
        }
    }
}

/// The result of one [`Request`], as the host's bridge reports it (`app_sync::Reply` without
/// the runtime's error type: a runtime failure arrives as its message).
#[derive(Debug, Clone)]
pub struct Reply {
    /// Why it was submitted.
    pub purpose: Purpose,
    /// Which command it was.
    pub name: CommandName,
    /// The engine's receipt or rejection, or the runtime failure's message.
    pub result: Result<Result<CommandReceipt, CoreError>, String>,
}
