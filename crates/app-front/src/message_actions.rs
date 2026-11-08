//! The actions of one message of a conversation (MISC-44): what a message card (GUI), the
//! reader's message cursor (TUI) and, later, the mobile presenter offer for that message alone,
//! each with the `Command` it submits.
//!
//! A message answers, forwards, toggles its read state and its star, and moves to the Trash on
//! its own (`messages: [id]`). A draft offers only Edit (`edit_draft`) and Discard: the local
//! draft saved as it is discarded (`discard_draft`), and a draft that is only on the server
//! (written on another device) is moved to the Trash. Either way the conversation stays.

use app_core::{AppState, Command, CommandName, Envelope, MessageId};

use crate::i18n::Key;

/// An action of a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageAction {
    /// Reply to it.
    Reply,
    /// Reply to it, to every recipient.
    ReplyAll,
    /// Forward it.
    Forward,
    /// Mark it read (it is unread).
    MarkRead,
    /// Mark it unread (it is read).
    MarkUnread,
    /// Star it (it has no star).
    Star,
    /// Remove its star.
    Unstar,
    /// Move it to the Trash.
    Trash,
    /// Edit the draft.
    Edit,
    /// Discard the draft.
    Discard,
}

impl MessageAction {
    /// Its name in the UI strings.
    pub const fn label(self) -> Key {
        match self {
            Self::Reply => Key::Reply,
            Self::ReplyAll => Key::ReplyAll,
            Self::Forward => Key::Forward,
            Self::MarkRead => Key::MarkRead,
            Self::MarkUnread => Key::MarkUnread,
            Self::Star => Key::Star,
            Self::Unstar => Key::Unstar,
            Self::Trash => Key::Trash,
            Self::Edit => Key::Edit,
            Self::Discard => Key::Discard,
        }
    }

    /// The registry command whose keys run it (for key hints): `r` replies to the message, `#`
    /// trashes or discards it, `u` and `s` toggle it.
    pub const fn key_command(self) -> CommandName {
        match self {
            Self::Reply => CommandName::Reply,
            Self::ReplyAll => CommandName::ReplyAll,
            Self::Forward => CommandName::Forward,
            Self::MarkRead => CommandName::MarkRead,
            Self::MarkUnread => CommandName::MarkUnread,
            Self::Star => CommandName::Star,
            Self::Unstar => CommandName::Unstar,
            Self::Trash | Self::Discard => CommandName::TrashThreads,
            Self::Edit => CommandName::EditDraft,
        }
    }

    /// Whether a key of the registry command `name` runs it. The read and star keys toggle,
    /// whichever of the pair they are bound to.
    pub fn answers(self, name: CommandName) -> bool {
        match self {
            Self::MarkRead | Self::MarkUnread => {
                matches!(name, CommandName::MarkRead | CommandName::MarkUnread)
            }
            Self::Star | Self::Unstar => matches!(name, CommandName::Star | CommandName::Unstar),
            action => action.key_command() == name,
        }
    }
}

/// An action and the command it submits.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// The action.
    pub action: MessageAction,
    /// What it submits.
    pub command: Command,
}

/// The actions of `message`, in display order; none if it is not cached.
pub fn message_actions(state: &AppState, message: MessageId) -> Vec<Item> {
    let Some(envelope) = state.messages.get(&message) else {
        return Vec::new();
    };
    if state.is_draft(envelope) {
        return draft_actions(state, envelope);
    }
    let messages = || vec![message];
    let item = |action, command| Item { action, command };
    vec![
        item(
            MessageAction::Reply,
            Command::Reply {
                message,
                body: None,
            },
        ),
        item(
            MessageAction::ReplyAll,
            Command::ReplyAll {
                message,
                body: None,
            },
        ),
        item(
            MessageAction::Forward,
            Command::Forward {
                message,
                body: None,
            },
        ),
        if envelope.flags.seen {
            item(
                MessageAction::MarkUnread,
                Command::MarkUnread {
                    ids: Vec::new(),
                    messages: messages(),
                },
            )
        } else {
            item(
                MessageAction::MarkRead,
                Command::MarkRead {
                    ids: Vec::new(),
                    messages: messages(),
                },
            )
        },
        if envelope.flags.flagged {
            item(
                MessageAction::Unstar,
                Command::Unstar {
                    ids: Vec::new(),
                    messages: messages(),
                },
            )
        } else {
            item(
                MessageAction::Star,
                Command::Star {
                    ids: Vec::new(),
                    messages: messages(),
                },
            )
        },
        item(MessageAction::Trash, trash(message)),
    ]
}

/// Edit and Discard of a draft message.
fn draft_actions(state: &AppState, envelope: &Envelope) -> Vec<Item> {
    let discard = match state.local_draft_of(envelope) {
        Some(id) => Command::DiscardDraft {
            id,
            keep_server_copy: false,
        },
        None => trash(envelope.id),
    };
    vec![
        Item {
            action: MessageAction::Edit,
            command: Command::EditDraft {
                message: envelope.id,
            },
        },
        Item {
            action: MessageAction::Discard,
            command: discard,
        },
    ]
}

fn trash(message: MessageId) -> Command {
    Command::TrashThreads {
        ids: Vec::new(),
        messages: vec![message],
    }
}

/// The command a key of the registry command `name` submits for `message` alone, if one of its
/// actions answers it (see [`MessageAction::answers`]).
pub fn for_key(state: &AppState, message: MessageId, name: CommandName) -> Option<Command> {
    message_actions(state, message)
        .into_iter()
        .find(|item| item.action.answers(name))
        .map(|item| item.command)
}
