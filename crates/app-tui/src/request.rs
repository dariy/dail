//! What the TUI submits and what comes back.

use app_core::{Command, ThreadId};

/// Why the TUI submitted a command; decides what happens with the result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Purpose {
    /// A user action: rejections and undo labels show in the status line.
    User,
    /// `get_thread` with bodies for the reader (`app_front::body`).
    Bodies(ThreadId),
    /// The account setup's `add_account` (its reply carries the new account's ID).
    Setup,
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

/// The result of one [`Request`].
pub type Reply = app_sync::Reply<Purpose>;
