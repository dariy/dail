//! The actions of threads, of a mailbox and of an account (PAR-1): one list each, which every
//! front end draws (the GUI's bulk bar, row and folder context menus and account buttons; the
//! mobile presenter's bulk bar and reader), so none of them hand-writes its own buttons.
//!
//! An [`Entry`] is a label, an icon name (an `app_theme::Icon::name()`; `app-front` does not
//! depend on `app-theme`, so a test in `app-gui` pins the names), what it runs ([`Run`]) and a
//! `primary` flag: the at most four entries a bar shows as icons when space is short.
//!
//! The account list has no *Servers…* entry yet: no flow edits `imap`/`smtp` of an account
//! (PAR-5 adds that form).

use app_core::{
    AccountId, AppState, Command, MailboxId, MailboxRole, MessageId, ThreadId, listing,
};

use crate::i18n::Key;
use crate::palette::{self, Flow};

/// What an entry runs: a command, or a palette flow with its explicit target.
///
/// Plain data (unlike [`Flow`], which holds closures), so lists compare in tests and front ends
/// store it in their click tables; [`Run::flow`] resolves it.
#[derive(Debug, Clone, PartialEq)]
pub enum Run {
    /// Submit this command.
    Command(Command),
    /// *Move…* these threads: asks for the target mailbox.
    Move(Vec<ThreadId>),
    /// *Make a rule…* from this message.
    RuleFrom(MessageId),
    /// *New subfolder…* under this mailbox.
    NewSubfolder(MailboxId),
    /// *Rename…* this mailbox.
    Rename(MailboxId),
    /// *Delete…* this mailbox (confirms).
    Delete(MailboxId),
    /// *Mark all read* in this mailbox (confirms when much is unread).
    MarkAllRead(MailboxId),
    /// Empty the Trash or the Junk folder of this account (confirms).
    Empty(AccountId, MailboxRole),
    /// *Offline window…* of this mailbox: asks for the number of messages.
    SyncWindow(MailboxId),
    /// The password form of this account.
    Password(AccountId),
    /// Add a sending address to this account: asks for it and its signature.
    Addresses(AccountId),
    /// Remove this account (confirms).
    RemoveAccount(AccountId),
}

impl Run {
    /// The palette flow it starts in `state`.
    pub fn flow(&self, state: &AppState) -> Flow {
        match self {
            Self::Command(cmd) => Flow::Run(cmd.clone()),
            Self::Move(ids) => palette::move_to(state, ids.clone()),
            Self::RuleFrom(message) => palette::rule_from(state, *message),
            Self::NewSubfolder(id) => palette::new_subfolder(state, *id),
            Self::Rename(id) => palette::rename_folder(state, *id),
            Self::Delete(id) => palette::delete_folder(state, *id),
            Self::MarkAllRead(id) => palette::mark_mailbox_read(state, *id),
            Self::Empty(account, role) => palette::empty_folder(state, *account, *role),
            Self::SyncWindow(id) => palette::sync_window(*id),
            Self::Password(id) => Flow::Password(*id),
            Self::Addresses(id) => palette::set_identity(*id),
            Self::RemoveAccount(id) => palette::remove_account(state, *id),
        }
    }
}

/// An action of a list.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Its name in the UI strings.
    pub label: Key,
    /// Its icon: an `app_theme::Icon::name()`.
    pub icon: &'static str,
    /// What it runs.
    pub run: Run,
    /// Shown as an icon in a bar when space is short (at most four per list).
    pub primary: bool,
}

fn entry(label: Key, icon: Icon, run: Run, primary: bool) -> Entry {
    Entry {
        label,
        icon: icon.name(),
        run,
        primary,
    }
}

fn cmd(label: Key, icon: Icon, command: Command, primary: bool) -> Entry {
    entry(label, icon, Run::Command(command), primary)
}

/// The actions of the threads `ids` (one or many), in display order; none without threads.
///
/// Archive, Trash, Junk (*Not junk* in a Junk folder), then one toggle each for read (*Mark
/// read* if any is unread), star (*Unstar* if every one is starred) and pin (*Unpin* if every
/// one is pinned), *Move…*, and for exactly one thread *Make a rule…* from its newest message.
pub fn thread_actions(state: &AppState, ids: &[ThreadId]) -> Vec<Entry> {
    let threads: Vec<_> = ids.iter().filter_map(|t| state.threads.get(t)).collect();
    if threads.is_empty() {
        return Vec::new();
    }
    let ids = || ids.to_vec();
    let messages = Vec::new;
    let envelopes = |t: &app_core::Thread| {
        t.messages
            .iter()
            .filter_map(|m| state.messages.get(m))
            .collect::<Vec<_>>()
    };
    let unread = threads
        .iter()
        .any(|t| envelopes(t).iter().any(|m| !m.flags.seen));
    let starred = threads
        .iter()
        .all(|t| envelopes(t).iter().any(|m| m.flags.flagged));
    let pinned = threads.iter().all(|t| listing::is_pinned(state, t));
    let junk = crate::mailbox::in_junk(state);
    let mut out = vec![
        cmd(
            Key::Archive,
            Icon::Archive,
            Command::ArchiveThreads {
                ids: ids(),
                messages: messages(),
            },
            true,
        ),
        cmd(
            Key::Trash,
            Icon::Delete,
            Command::TrashThreads {
                ids: ids(),
                messages: messages(),
            },
            true,
        ),
        cmd(
            if junk { Key::NotJunk } else { Key::MarkJunk },
            if junk { Icon::Inbox } else { Icon::Report },
            crate::mailbox::junk_command(state, ids()),
            false,
        ),
        if unread {
            cmd(
                Key::MarkRead,
                Icon::MarkEmailRead,
                Command::MarkRead {
                    ids: ids(),
                    messages: messages(),
                },
                true,
            )
        } else {
            cmd(
                Key::MarkUnread,
                Icon::MarkEmailUnread,
                Command::MarkUnread {
                    ids: ids(),
                    messages: messages(),
                },
                true,
            )
        },
        if starred {
            cmd(
                Key::Unstar,
                Icon::Star,
                Command::Unstar {
                    ids: ids(),
                    messages: messages(),
                },
                false,
            )
        } else {
            cmd(
                Key::Star,
                Icon::Star,
                Command::Star {
                    ids: ids(),
                    messages: messages(),
                },
                false,
            )
        },
        if pinned {
            cmd(
                Key::Unpin,
                Icon::PushPin,
                Command::UnpinThreads {
                    ids: ids(),
                    messages: messages(),
                },
                false,
            )
        } else {
            cmd(
                Key::Pin,
                Icon::PushPin,
                Command::PinThreads {
                    ids: ids(),
                    messages: messages(),
                },
                false,
            )
        },
        entry(Key::Move, Icon::DriveFileMove, Run::Move(ids()), true),
    ];
    if let [thread] = threads.as_slice()
        && let Some(message) = palette::newest_message(state, thread.id)
    {
        out.push(entry(
            Key::MakeRule,
            Icon::Edit,
            Run::RuleFrom(message),
            false,
        ));
    }
    out
}

/// *Select all* for a bar acting on the checked threads (TCH-3): checks every thread the list
/// shows, as the desktop's header checkbox does (`check_threads` without ids). Not a thread
/// action (it does not act on given ids), and none when the list shows nothing or every shown
/// thread is checked already.
pub fn select_all(state: &AppState) -> Option<Entry> {
    let shown = listing::visible_threads(state);
    let all = shown.iter().all(|id| state.view.checked.contains(id));
    (!all).then(|| {
        cmd(
            Key::SelectAll,
            Icon::CheckBox,
            Command::CheckThreads { ids: Vec::new() },
            false,
        )
    })
}

/// The actions of mailbox `id`, in display order; none if it is unknown.
///
/// *New subfolder…*, *Rename…* (not the Inbox), pin or unpin in the favourites, show or hide,
/// focus, *Mark all read*, *Empty Trash* / *Empty Junk* (only those two), *Apply rules*,
/// *Offline window…* and *Delete…* (not the Inbox).
pub fn mailbox_actions(state: &AppState, id: MailboxId) -> Vec<Entry> {
    let Some(mailbox) = state.mailboxes.get(&id) else {
        return Vec::new();
    };
    let key = mailbox.key();
    let pinned = state.settings.pinned_mailboxes.contains(&key);
    let hidden = state.settings.hidden_mailboxes.contains(&key);
    let inbox = mailbox.role == Some(MailboxRole::Inbox);
    let mut out = vec![entry(
        Key::NewSubfolder,
        Icon::Add,
        Run::NewSubfolder(id),
        true,
    )];
    if !inbox {
        out.push(entry(Key::Rename, Icon::Edit, Run::Rename(id), false));
    }
    out.push(if pinned {
        cmd(
            Key::UnpinFavourites,
            Icon::PushPin,
            Command::UnpinMailbox { id },
            true,
        )
    } else {
        cmd(
            Key::PinFavourites,
            Icon::PushPin,
            Command::PinMailbox { id },
            true,
        )
    });
    out.push(if hidden {
        cmd(
            Key::ShowFolder,
            Icon::Visibility,
            Command::ShowMailbox { id },
            false,
        )
    } else {
        cmd(
            Key::HideFolder,
            Icon::VisibilityOff,
            Command::HideMailbox { id },
            false,
        )
    });
    out.push(cmd(
        Key::FocusMailbox,
        Icon::OpenInFull,
        Command::FocusMailbox {
            id: Some(id),
            account: None,
        },
        false,
    ));
    out.push(entry(
        Key::MarkAllRead,
        Icon::MarkEmailRead,
        Run::MarkAllRead(id),
        true,
    ));
    let empty = match mailbox.role {
        Some(role @ MailboxRole::Trash) => Some((role, Key::EmptyTrash)),
        Some(role @ MailboxRole::Junk) => Some((role, Key::EmptyJunk)),
        _ => None,
    };
    if let Some((role, label)) = empty {
        out.push(entry(
            label,
            Icon::Delete,
            Run::Empty(mailbox.account, role),
            true,
        ));
    }
    out.push(cmd(
        Key::ApplyRules,
        Icon::DriveFileMove,
        Command::ApplyRules { mailbox: id },
        false,
    ));
    out.push(entry(
        Key::OfflineWindow,
        Icon::CloudOff,
        Run::SyncWindow(id),
        false,
    ));
    if !inbox {
        out.push(entry(Key::Delete, Icon::Delete, Run::Delete(id), false));
    }
    out
}

/// The actions of account `id`, in display order; none if it is unknown: test, the password
/// form, a sending address, remove (confirms). No *Servers…* yet (PAR-5).
pub fn account_actions(state: &AppState, id: AccountId) -> Vec<Entry> {
    if !state.accounts.contains_key(&id) {
        return Vec::new();
    }
    vec![
        cmd(
            Key::Test,
            Icon::VerifiedUser,
            Command::TestAccount { id },
            true,
        ),
        entry(
            Key::ReEnterPassword,
            Icon::Keyboard,
            Run::Password(id),
            true,
        ),
        entry(
            Key::SendingAddresses,
            Icon::Person,
            Run::Addresses(id),
            false,
        ),
        entry(Key::RemoveDots, Icon::Delete, Run::RemoveAccount(id), false),
    ]
}

/// The icons the lists use: the only way an entry gets one, so [`ICONS`] holds every name a
/// list can return.
macro_rules! icons {
    ($($variant:ident => $name:literal,)*) => {
        /// An icon of an entry; [`Icon::name`] is an `app_theme::Icon::name()`.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Icon {
            $(
                #[doc = $name]
                $variant,
            )*
        }

        impl Icon {
            /// Its `app_theme` name.
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)*
                }
            }
        }

        /// Every icon name the lists can return (a test pins them to `app_theme` icons).
        pub const ICONS: &[&str] = &[$($name,)*];
    };
}

icons! {
    Archive => "archive",
    Delete => "delete",
    Inbox => "inbox",
    Report => "report",
    MarkEmailRead => "mark_email_read",
    MarkEmailUnread => "mark_email_unread",
    Star => "star",
    PushPin => "push_pin",
    DriveFileMove => "drive_file_move",
    Edit => "edit",
    Add => "add",
    Visibility => "visibility",
    VisibilityOff => "visibility_off",
    OpenInFull => "open_in_full",
    CloudOff => "cloud_off",
    VerifiedUser => "verified_user",
    Keyboard => "keyboard",
    Person => "person",
    CheckBox => "check_box",
}
