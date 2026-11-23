//! The actions of threads, of a mailbox and of an account (PAR-1): one list each, which every
//! front end draws (the GUI's bulk bar, row and folder context menus and account buttons; the
//! mobile presenter's bulk bar and reader), so none of them hand-writes its own buttons. The
//! dail server's lists (ADR 0017, SRV-6) are drawn by the GUI and the TUI: the link's
//! ([`link_actions`]), a refused offline command's ([`rejection_actions`]), a paired device's
//! ([`device_actions`]) and the server's own ([`server_actions`]).
//!
//! An [`Entry`] is a label, an icon name (an `app_theme::Icon::name()`; `app-front` does not
//! depend on `app-theme`, so a test in `app-gui` pins the names), what it runs ([`Run`]) and a
//! `primary` flag: the at most four entries a bar shows as icons when space is short.
//!
//! The account list has no *Servers…* entry yet: no flow edits `imap`/`smtp` of an account
//! (PAR-5 adds that form).

use app_core::{
    AccountId, AppState, Command, DeviceId, MailboxId, MailboxRole, MessageId, RequestId, ThreadId,
    listing,
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
    /// Trust a certificate one of this account's servers offered: picks it (ADR 0016).
    TrustCertificate(AccountId),
    /// Forget a certificate this account trusts: picks it.
    ForgetCertificate(AccountId),
    /// *Pair a device…*: the GUI and the TUI open their pairing dialog
    /// ([`crate::pairing::PairDialog`]) instead of [`Run::flow`], whose palette questions are
    /// the fallback of a front end without one.
    Pair,
    /// *Revoke…* this paired device (confirms).
    Revoke(DeviceId),
    /// *Disconnect…* from the dail server (confirms).
    Disconnect,
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
            Self::TrustCertificate(id) => palette::trust_certificate(state, Some(*id)),
            Self::ForgetCertificate(id) => palette::forget_certificate(state, Some(*id)),
            Self::Pair => palette::start_pairing(),
            Self::Revoke(id) => palette::revoke_this_device(state, *id),
            Self::Disconnect => palette::disconnect_server(state),
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
/// form, a sending address, *Trust certificate…* while a server offers one and *Forget
/// certificate…* while it trusts one (ADR 0016), remove (confirms). No *Servers…* yet (PAR-5).
pub fn account_actions(state: &AppState, id: AccountId) -> Vec<Entry> {
    let Some(account) = state.accounts.get(&id) else {
        return Vec::new();
    };
    let mut certs = Vec::new();
    if !crate::certs::offers(state, id).is_empty() {
        certs.push(entry(
            Key::TrustCertificateDots,
            Icon::VerifiedUser,
            Run::TrustCertificate(id),
            false,
        ));
    }
    if !account.trusted_certs.is_empty() {
        certs.push(entry(
            Key::ForgetCertificateDots,
            Icon::Delete,
            Run::ForgetCertificate(id),
            false,
        ));
    }
    let mut out = vec![
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
    ];
    out.extend(certs);
    out.push(entry(
        Key::RemoveDots,
        Icon::Delete,
        Run::RemoveAccount(id),
        false,
    ));
    out
}

/// The actions of this dail server or of the server this profile mirrors (ADR 0017): *Pair a
/// device…* while a pairing code can be opened ([`crate::pairing::can_pair`]). The GUI's and the
/// TUI's *Devices* settings show them (SRV-6).
pub fn server_actions(state: &AppState) -> Vec<Entry> {
    if crate::pairing::can_pair(state) {
        vec![entry(Key::PairADevice, Icon::Link, Run::Pair, true)]
    } else {
        Vec::new()
    }
}

/// The actions of this device's link to its dail server (SRV-6), none without a link: *Retry
/// refused changes* while the server refused some (`retry_offline` queues them all again), then
/// *Disconnect…* (confirms). The status bar's link details and the settings show them.
pub fn link_actions(state: &AppState) -> Vec<Entry> {
    let Some(link) = state.link.as_ref() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if !link.rejected.is_empty() {
        out.push(cmd(
            Key::RetryRefused,
            Icon::Sync,
            Command::RetryOffline {},
            true,
        ));
    }
    out.push(entry(
        Key::DisconnectDots,
        Icon::CloudOff,
        Run::Disconnect,
        false,
    ));
    out
}

/// The actions of a command the server refused after an offline period (`id` in
/// `link.rejected`), none if it is unknown: *Retry all* (`retry_offline` has no per-command form:
/// it queues every refused command again, so the label says all) and *Dismiss*. The problems
/// list shows them next to the refusal.
pub fn rejection_actions(state: &AppState, id: RequestId) -> Vec<Entry> {
    let known = state
        .link
        .as_ref()
        .is_some_and(|link| link.rejected.iter().any(|r| r.request_id == id));
    if !known {
        return Vec::new();
    }
    vec![
        cmd(Key::RetryAll, Icon::Sync, Command::RetryOffline {}, true),
        cmd(
            Key::Dismiss,
            Icon::Close,
            Command::DismissOfflineRejection { id },
            true,
        ),
    ]
}

/// The actions of a device paired with the dail server, none if it is unknown: *Revoke…*
/// (confirms). The *Devices* settings show them.
pub fn device_actions(state: &AppState, id: DeviceId) -> Vec<Entry> {
    if state.devices.contains_key(&id) {
        vec![entry(Key::RevokeDots, Icon::Delete, Run::Revoke(id), false)]
    } else {
        Vec::new()
    }
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
    Link => "link",
    Sync => "sync",
    Close => "close",
}
