//! The command palette's logic, shared by the GUI and the TUI: fuzzy search over the available
//! commands of the registry, the mailboxes and recent threads, then second-step pickers for the
//! arguments a command needs.
//!
//! Entries come from `Command::specs()`; [`flow`] says how each command gets its arguments.
//! It is an exhaustive `match` over [`CommandName`], so a new command does not compile until the
//! palette knows how to build it (the "appears in the palette" parity of `ARCHITECTURE.md
//! §4.5`). [`Palette`] is the state of an open palette without any drawing: the query, the
//! cursor and the current step.

use std::fmt;

use app_core::{
    AccountId, AppState, ApprovalId, Category, Command, CommandName, DraftId, MailboxId, MessageId,
    OpId, Pane, SettingKey, ThreadId, ThreadSummary, listing,
};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

use crate::format;
use crate::mailbox::mailbox_name;

/// How many recent threads the palette offers.
const RECENT_THREADS: usize = 20;

/// Mailbox roles for the `g <letter>` bindings of `open_mailbox`.
pub const GO_TO: &[(char, app_core::MailboxRole)] = &[
    ('i', app_core::MailboxRole::Inbox),
    ('s', app_core::MailboxRole::Sent),
    ('d', app_core::MailboxRole::Drafts),
    ('a', app_core::MailboxRole::Archive),
    ('t', app_core::MailboxRole::Trash),
];

/// A value picked in a second step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    /// A mailbox.
    Mailbox(MailboxId),
    /// A thread.
    Thread(ThreadId),
    /// A draft.
    Draft(DraftId),
    /// An account.
    Account(AccountId),
    /// A pending op.
    Op(OpId),
    /// An agent's send request.
    Approval(ApprovalId),
    /// A setting.
    Setting(SettingKey),
    /// A pane.
    Pane(Pane),
    /// A position in a list (e.g. an attachment).
    Index(u32),
    /// Typed text.
    Text(String),
    /// Confirmation of a destructive step.
    Yes,
}

/// What a second step offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AskKind {
    /// Mailboxes, of one account if given.
    Mailboxes(Option<AccountId>),
    /// Recent threads.
    Threads,
    /// Drafts; only scheduled ones with `scheduled`.
    Drafts {
        /// Only drafts whose send is scheduled.
        scheduled: bool,
    },
    /// Accounts.
    Accounts,
    /// Pending ops.
    Ops,
    /// Agents' send requests.
    Approvals,
    /// Settings.
    Settings,
    /// Panes that can have the focus.
    Panes,
    /// A draft's attachments.
    Attachments(DraftId),
    /// Free text; the hint is shown in the input.
    Text(String),
    /// Yes / no.
    Confirm(String),
}

type Next = Box<dyn Fn(&AppState, Choice) -> Flow>;

/// A second step: what to ask and what to do with the answer.
pub struct Ask {
    /// Shown above the choices.
    pub prompt: String,
    /// What to offer.
    pub kind: AskKind,
    next: Next,
}

impl Ask {
    /// Continues the flow with `choice`.
    pub fn answer(&self, state: &AppState, choice: Choice) -> Flow {
        (self.next)(state, choice)
    }
}

impl fmt::Debug for Ask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Ask")
            .field("prompt", &self.prompt)
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

/// What choosing a command in the palette leads to.
#[derive(Debug)]
pub enum Flow {
    /// Submit this command.
    Run(Command),
    /// Ask for an argument first.
    Ask(Ask),
    /// Open the account wizard (adding an account needs a password form).
    Wizard,
    /// Open the password form for this account (`set_password`; a password is never typed
    /// into the palette).
    Password(app_core::AccountId),
    /// Not offered in the palette.
    Hidden,
}

fn ask(
    prompt: impl Into<String>,
    kind: AskKind,
    next: impl Fn(&AppState, Choice) -> Flow + 'static,
) -> Flow {
    Flow::Ask(Ask {
        prompt: prompt.into(),
        kind,
        next: Box::new(next),
    })
}

/// The threads a triage command acts on: the checked ones, else the selection, else the open
/// thread.
pub fn targets(state: &AppState) -> Vec<ThreadId> {
    if !state.view.checked.is_empty() {
        state.view.checked.clone()
    } else if state.view.selected.is_empty() {
        state.view.thread.into_iter().collect()
    } else {
        state.view.selected.clone()
    }
}

/// The newest message of a thread.
pub fn newest_message(state: &AppState, thread: ThreadId) -> Option<MessageId> {
    state.threads.get(&thread)?.messages.last().copied()
}

/// The sender of the open thread's newest message.
pub fn open_sender(state: &AppState) -> Option<String> {
    let message = newest_message(state, state.view.thread?)?;
    let from = state.messages.get(&message)?.from.first()?;
    Some(from.email.clone())
}

/// The account new mail is written from: the open mailbox's, else the first.
pub fn current_account(state: &AppState) -> Option<AccountId> {
    state
        .view
        .mailbox
        .and_then(|m| state.mailboxes.get(&m))
        .map(|m| m.account)
        .or_else(|| state.accounts.keys().next().copied())
}

/// Runs `make(threads)` on the targets, or asks for a thread first.
fn on_threads(state: &AppState, make: fn(Vec<ThreadId>) -> Command) -> Flow {
    let ids = targets(state);
    if ids.is_empty() {
        ask(
            "Which conversation?",
            AskKind::Threads,
            move |_, choice| match choice {
                Choice::Thread(id) => Flow::Run(make(vec![id])),
                _ => Flow::Hidden,
            },
        )
    } else {
        Flow::Run(make(ids))
    }
}

/// Runs `make(message)` on the open thread's newest message, or asks for a thread first.
fn on_message(state: &AppState, make: fn(MessageId) -> Command) -> Flow {
    if let Some(message) = state.view.thread.and_then(|t| newest_message(state, t)) {
        return Flow::Run(make(message));
    }
    ask(
        "Which conversation?",
        AskKind::Threads,
        move |state, choice| match choice {
            Choice::Thread(id) => {
                newest_message(state, id).map_or(Flow::Hidden, |m| Flow::Run(make(m)))
            }
            _ => Flow::Hidden,
        },
    )
}

/// Runs `make(draft)` on the composer's draft, or asks for one.
fn on_draft(state: &AppState, scheduled: bool, make: fn(DraftId) -> Command) -> Flow {
    if let Some(id) = state.view.composer
        && !scheduled
    {
        return Flow::Run(make(id));
    }
    ask(
        "Which draft?",
        AskKind::Drafts { scheduled },
        move |_, choice| match choice {
            Choice::Draft(id) => Flow::Run(make(id)),
            _ => Flow::Hidden,
        },
    )
}

fn pick_mailbox(
    prompt: &str,
    account: Option<AccountId>,
    make: impl Fn(MailboxId) -> Command + 'static,
) -> Flow {
    ask(
        prompt,
        AskKind::Mailboxes(account),
        move |_, choice| match choice {
            Choice::Mailbox(id) => Flow::Run(make(id)),
            _ => Flow::Hidden,
        },
    )
}

fn pick_account(make: impl Fn(&AppState, AccountId) -> Flow + 'static) -> Flow {
    ask(
        "Which account?",
        AskKind::Accounts,
        move |state, choice| match choice {
            Choice::Account(id) => make(state, id),
            _ => Flow::Hidden,
        },
    )
}

/// The `set_theme` that typed text asks for: a mode (`system`, `light`, `dark`), then optionally
/// a theme name for that mode (`dark dail-dark-hc`); a name alone sets the mode its name says
/// (`…-light…` light, else dark). Anything else gives a `set_theme` the engine rejects.
pub fn theme_command(text: &str) -> Command {
    let mut words = text.split_whitespace();
    let first = words.next().unwrap_or_default().to_lowercase();
    let mode = match first.as_str() {
        "system" => Some(app_core::ThemeMode::System),
        "light" => Some(app_core::ThemeMode::Light),
        "dark" => Some(app_core::ThemeMode::Dark),
        _ => None,
    };
    let name = if mode.is_some() {
        words.next().map(str::to_lowercase)
    } else {
        (!first.is_empty()).then_some(first)
    };
    let light_name = match (mode, &name) {
        (Some(app_core::ThemeMode::Light), _) => true,
        (None, Some(name)) => name.contains("light"),
        _ => false,
    };
    match name {
        Some(name) if light_name => Command::SetTheme {
            mode,
            light: Some(name),
            dark: None,
        },
        Some(name) if mode != Some(app_core::ThemeMode::System) => Command::SetTheme {
            mode,
            light: None,
            dark: Some(name),
        },
        _ => Command::SetTheme {
            mode,
            light: None,
            dark: None,
        },
    }
}

fn text(prompt: &str, hint: impl Into<String>, make: impl Fn(String) -> Command + 'static) -> Flow {
    ask(
        prompt,
        AskKind::Text(hint.into()),
        move |_, choice| match choice {
            Choice::Text(text) if !text.trim().is_empty() => {
                Flow::Run(make(text.trim().to_owned()))
            }
            _ => Flow::Hidden,
        },
    )
}

/// How the palette builds `name` in `state`.
pub fn flow(name: CommandName, state: &AppState) -> Flow {
    match name {
        CommandName::GetSettings => Flow::Run(Command::GetSettings {}),
        CommandName::DiscoverServers => text(
            "Find the mail servers of which address?",
            "you@example.org (asks the provider, then Mozilla)",
            |email| Command::DiscoverServers { email },
        ),
        CommandName::SetKeyPreset => text("Which keys?", "gmail or vim", |text| {
            Command::SetKeyPreset {
                preset: if text.trim().eq_ignore_ascii_case("vim") {
                    app_core::KeyPreset::Vim
                } else {
                    app_core::KeyPreset::Gmail
                },
            }
        }),
        CommandName::SetTheme => text(
            "Which theme?",
            "system, light or dark, then a theme for that mode: dark dail-dark-hc",
            |text| theme_command(&text),
        ),
        CommandName::SetKeybinding => ask(
            "Which command?",
            AskKind::Text("its name, e.g. archive_threads".into()),
            |_, choice| match choice {
                Choice::Text(command) if !command.trim().is_empty() => {
                    let command = command.trim().to_owned();
                    ask(
                        "Which keys? (comma-separated; empty: the preset's)",
                        AskKind::Text("e, Shift+E, g a".into()),
                        move |_, choice| match choice {
                            Choice::Text(keys) => {
                                let keys: Vec<String> = keys
                                    .split(',')
                                    .map(|k| k.trim().to_owned())
                                    .filter(|k| !k.is_empty())
                                    .collect();
                                Flow::Run(Command::SetKeybinding {
                                    command: command.clone(),
                                    keys: (!keys.is_empty()).then_some(keys),
                                })
                            }
                            _ => Flow::Hidden,
                        },
                    )
                }
                _ => Flow::Hidden,
            },
        ),
        CommandName::SetSetting => ask("Which setting?", AskKind::Settings, |state, choice| {
            let Choice::Setting(key) = choice else {
                return Flow::Hidden;
            };
            let (min, max) = key.range();
            let current = key.get(&state.settings);
            ask(
                format!("New value for {}", key.as_str()),
                AskKind::Text(format!("{min}–{max}, now {current}")),
                move |_, choice| match choice {
                    Choice::Text(text) => {
                        text.trim().parse::<u64>().map_or(Flow::Hidden, |value| {
                            Flow::Run(Command::SetSetting {
                                key,
                                value: value.into(),
                            })
                        })
                    }
                    _ => Flow::Hidden,
                },
            )
        }),
        CommandName::Undo => Flow::Run(Command::Undo {}),
        CommandName::Redo => Flow::Run(Command::Redo {}),
        // The palette is already open.
        CommandName::OpenPalette => Flow::Hidden,
        CommandName::ClosePalette => Flow::Run(Command::ClosePalette {}),
        CommandName::AddAccount => Flow::Wizard,
        CommandName::TestAccount => pick_account(|_, id| Flow::Run(Command::TestAccount { id })),
        CommandName::SetPassword => pick_account(|_, id| Flow::Password(id)),
        CommandName::UpdateAccount => pick_account(|state, id| {
            let current = state
                .accounts
                .get(&id)
                .map(|a| a.display_name.clone())
                .unwrap_or_default();
            text("New display name", current, move |name| {
                Command::UpdateAccount {
                    id,
                    display_name: Some(name),
                    imap: None,
                    smtp: None,
                }
            })
        }),
        CommandName::RemoveAccount => pick_account(|state, id| {
            let email = state
                .accounts
                .get(&id)
                .map(|a| a.email.clone())
                .unwrap_or_default();
            ask(
                "Remove this account? This cannot be undone.",
                AskKind::Confirm(format!("Remove {email} and its password")),
                move |_, choice| match choice {
                    Choice::Yes => Flow::Run(Command::RemoveAccount { id, confirm: true }),
                    _ => Flow::Hidden,
                },
            )
        }),
        CommandName::SyncNow => Flow::Run(Command::SyncNow { account: None }),
        CommandName::ListMailboxes => Flow::Run(Command::ListMailboxes { account: None }),
        CommandName::ListThreads => {
            pick_mailbox("Which mailbox?", None, |mailbox| Command::ListThreads {
                mailbox,
                offset: 0,
                limit: 50,
            })
        }
        CommandName::SetSyncWindow => {
            let window = |mailbox| {
                ask(
                    "Keep how many of the newest messages offline?",
                    AskKind::Text("a number; 0 keeps all of it".into()),
                    move |_, choice| match choice {
                        Choice::Text(text) => {
                            text.trim().parse::<u32>().map_or(Flow::Hidden, |n| {
                                Flow::Run(Command::SetSyncWindow {
                                    mailbox,
                                    window: Some(n),
                                })
                            })
                        }
                        _ => Flow::Hidden,
                    },
                )
            };
            match state.view.mailbox {
                Some(mailbox) => window(mailbox),
                None => ask(
                    "Which mailbox?",
                    AskKind::Mailboxes(None),
                    move |_, choice| match choice {
                        Choice::Mailbox(id) => window(id),
                        _ => Flow::Hidden,
                    },
                ),
            }
        }
        CommandName::OpenUnifiedInbox => Flow::Run(Command::OpenUnifiedInbox {}),
        CommandName::OpenMailbox => {
            pick_mailbox("Go to mailbox", None, |id| Command::OpenMailbox { id })
        }
        CommandName::SetIdentity => pick_account(|_, account| {
            ask(
                "Which address?",
                AskKind::Text("sales@example.org".into()),
                move |_, choice| match choice {
                    Choice::Text(email) if !email.trim().is_empty() => {
                        let email = email.trim().to_owned();
                        ask(
                            "Its signature? (empty: none)",
                            AskKind::Text("Name · Company".into()),
                            move |_, choice| match choice {
                                Choice::Text(signature) => Flow::Run(Command::SetIdentity {
                                    account,
                                    email: email.clone(),
                                    name: None,
                                    signature: Some(signature.trim().to_owned())
                                        .filter(|s| !s.is_empty()),
                                }),
                                _ => Flow::Hidden,
                            },
                        )
                    }
                    _ => Flow::Hidden,
                },
            )
        }),
        CommandName::RemoveIdentity => pick_account(|_, account| {
            text("Remove which address?", "sales@example.org", move |email| {
                Command::RemoveIdentity { account, email }
            })
        }),
        CommandName::SetMailboxIdentity => {
            let email = |mailbox| {
                ask(
                    "Send from which address? (empty: the account's own)",
                    AskKind::Text("sales@example.org".into()),
                    move |_, choice| match choice {
                        Choice::Text(text) => Flow::Run(Command::SetMailboxIdentity {
                            mailbox,
                            email: Some(text.trim().to_owned()).filter(|e| !e.is_empty()),
                        }),
                        _ => Flow::Hidden,
                    },
                )
            };
            match state.view.mailbox {
                Some(mailbox) => email(mailbox),
                None => ask(
                    "Which mailbox?",
                    AskKind::Mailboxes(None),
                    move |_, choice| match choice {
                        Choice::Mailbox(id) => email(id),
                        _ => Flow::Hidden,
                    },
                ),
            }
        }
        CommandName::SetDraftFrom => match state.view.composer {
            Some(id) => text(
                "Send from which address?",
                "sales@example.org",
                move |email| Command::SetDraftFrom { id, email },
            ),
            None => ask(
                "Which draft?",
                AskKind::Drafts { scheduled: false },
                |_, choice| match choice {
                    Choice::Draft(id) => text(
                        "Send from which address?",
                        "sales@example.org",
                        move |email| Command::SetDraftFrom { id, email },
                    ),
                    _ => Flow::Hidden,
                },
            ),
        },
        CommandName::ListContacts => text(
            "List contacts matching…",
            "a name or an address",
            |query| Command::ListContacts { query, limit: 50 },
        ),
        CommandName::CheckThreads => Flow::Run(Command::CheckThreads { ids: Vec::new() }),
        CommandName::UncheckThreads => Flow::Run(Command::UncheckThreads { ids: Vec::new() }),
        CommandName::SelectThreads => ask(
            "Select which conversation?",
            AskKind::Threads,
            |_, choice| match choice {
                Choice::Thread(id) => Flow::Run(Command::SelectThreads { ids: vec![id] }),
                _ => Flow::Hidden,
            },
        ),
        CommandName::OpenThread => ask(
            "Open which conversation?",
            AskKind::Threads,
            |_, choice| match choice {
                Choice::Thread(id) => Flow::Run(Command::OpenThread { id }),
                _ => Flow::Hidden,
            },
        ),
        CommandName::CloseThread => Flow::Run(Command::CloseThread {}),
        CommandName::ExpandMessages => Flow::Run(Command::ExpandMessages { ids: Vec::new() }),
        CommandName::CollapseMessages => Flow::Run(Command::CollapseMessages { ids: Vec::new() }),
        CommandName::GetThread => match state.view.thread {
            Some(id) => Flow::Run(Command::GetThread {
                id,
                include_body: false,
                rich_body: false,
            }),
            None => ask(
                "Which conversation?",
                AskKind::Threads,
                |_, choice| match choice {
                    Choice::Thread(id) => Flow::Run(Command::GetThread {
                        id,
                        include_body: false,
                        rich_body: false,
                    }),
                    _ => Flow::Hidden,
                },
            ),
        },
        CommandName::GetMessage => on_message(state, |id| Command::GetMessage {
            id,
            include_body: false,
        }),
        CommandName::ShowImagesFrom => match open_sender(state) {
            Some(sender) => Flow::Run(Command::ShowImagesFrom { sender }),
            None => text(
                "Show images from which sender?",
                "news@example.org",
                |sender| Command::ShowImagesFrom { sender },
            ),
        },
        CommandName::HideImagesFrom => {
            match open_sender(state).filter(|s| state.settings.shows_images_from(s)) {
                Some(sender) => Flow::Run(Command::HideImagesFrom { sender }),
                None => text(
                    "Hide images from which sender?",
                    "news@example.org",
                    |sender| Command::HideImagesFrom { sender },
                ),
            }
        }
        CommandName::SaveAttachments => {
            let folder = |message| {
                text(
                    "Save the attachments into which folder?",
                    "an absolute path, e.g. /home/me/Downloads",
                    move |dir| Command::SaveAttachments {
                        message,
                        parts: Vec::new(),
                        dir,
                    },
                )
            };
            match state.view.thread.and_then(|t| newest_message(state, t)) {
                Some(message) => folder(message),
                None => ask(
                    "Which conversation?",
                    AskKind::Threads,
                    move |state, choice| match choice {
                        Choice::Thread(id) => {
                            newest_message(state, id).map_or(Flow::Hidden, folder)
                        }
                        _ => Flow::Hidden,
                    },
                ),
            }
        }
        CommandName::MarkRead => on_threads(state, |ids| Command::MarkRead {
            ids,
            messages: Vec::new(),
        }),
        CommandName::MarkUnread => on_threads(state, |ids| Command::MarkUnread {
            ids,
            messages: Vec::new(),
        }),
        CommandName::Star => on_threads(state, |ids| Command::Star {
            ids,
            messages: Vec::new(),
        }),
        CommandName::Unstar => on_threads(state, |ids| Command::Unstar {
            ids,
            messages: Vec::new(),
        }),
        CommandName::ArchiveThreads => on_threads(state, |ids| Command::ArchiveThreads {
            ids,
            messages: Vec::new(),
        }),
        CommandName::TrashThreads => on_threads(state, |ids| Command::TrashThreads {
            ids,
            messages: Vec::new(),
        }),
        CommandName::MoveThreads => {
            let then = |state: &AppState, ids: Vec<ThreadId>| {
                let account = ids
                    .first()
                    .and_then(|t| state.threads.get(t))
                    .map(|t| t.account);
                pick_mailbox("Move to…", account, move |to| Command::MoveThreads {
                    ids: ids.clone(),
                    messages: Vec::new(),
                    to,
                })
            };
            let ids = targets(state);
            if ids.is_empty() {
                ask(
                    "Move which conversation?",
                    AskKind::Threads,
                    move |state, choice| match choice {
                        Choice::Thread(id) => then(state, vec![id]),
                        _ => Flow::Hidden,
                    },
                )
            } else {
                then(state, ids)
            }
        }
        CommandName::ListPendingOps => Flow::Run(Command::ListPendingOps { account: None }),
        CommandName::RetryPendingOps => Flow::Run(Command::RetryPendingOps { id: None }),
        CommandName::DiscardPendingOp => {
            ask(
                "Discard which change?",
                AskKind::Ops,
                |_, choice| match choice {
                    Choice::Op(id) => Flow::Run(Command::DiscardPendingOp { id }),
                    _ => Flow::Hidden,
                },
            )
        }
        CommandName::CreateDraft => {
            let make = |account| {
                Flow::Run(Command::CreateDraft {
                    account,
                    to: Vec::new(),
                    cc: Vec::new(),
                    bcc: Vec::new(),
                    subject: String::new(),
                    body: String::new(),
                })
            };
            match current_account(state) {
                Some(account) if state.accounts.len() == 1 || state.view.mailbox.is_some() => {
                    make(account)
                }
                _ => pick_account(move |_, id| make(id)),
            }
        }
        CommandName::Reply => on_message(state, |message| Command::Reply {
            message,
            body: None,
        }),
        CommandName::ReplyAll => on_message(state, |message| Command::ReplyAll {
            message,
            body: None,
        }),
        CommandName::Forward => on_message(state, |message| Command::Forward {
            message,
            body: None,
        }),
        // Drafts are edited in the composer.
        CommandName::UpdateDraft => on_draft(state, false, |id| Command::OpenComposer { id }),
        CommandName::DiscardDraft => on_draft(state, false, |id| Command::DiscardDraft { id }),
        // Only `undo` restores drafts.
        CommandName::RestoreDraft => Flow::Hidden,
        CommandName::AttachFile => {
            let then = |draft: DraftId| {
                text(
                    "Attach which file?",
                    "absolute path, e.g. /home/me/report.pdf",
                    move |path| Command::AttachFile {
                        draft,
                        path,
                        index: None,
                    },
                )
            };
            match state.view.composer {
                Some(draft) => then(draft),
                None => ask(
                    "Attach to which draft?",
                    AskKind::Drafts { scheduled: false },
                    move |_, choice| match choice {
                        Choice::Draft(id) => then(id),
                        _ => Flow::Hidden,
                    },
                ),
            }
        }
        CommandName::RemoveAttachment => {
            let then = |draft: DraftId| {
                ask(
                    "Remove which attachment?",
                    AskKind::Attachments(draft),
                    move |_, choice| match choice {
                        Choice::Index(index) => {
                            Flow::Run(Command::RemoveAttachment { draft, index })
                        }
                        _ => Flow::Hidden,
                    },
                )
            };
            match state.view.composer {
                Some(draft) => then(draft),
                None => ask(
                    "From which draft?",
                    AskKind::Drafts { scheduled: false },
                    move |_, choice| match choice {
                        Choice::Draft(id) => then(id),
                        _ => Flow::Hidden,
                    },
                ),
            }
        }
        CommandName::SendDraft => on_draft(state, false, |id| Command::SendDraft { id }),
        CommandName::CancelSend => on_draft(state, true, |id| Command::CancelSend { id }),
        CommandName::Search => text(
            "Search mail (results as data)",
            "from:alice is:unread",
            |query| Command::Search {
                query,
                account: None,
                offset: 0,
                limit: 50,
                server: true,
                order: app_core::SearchOrder::default(),
            },
        ),
        CommandName::SetSearch => text(
            "Search",
            "from:alice has:attachment after:2024-01-01",
            |query| Command::SetSearch { query },
        ),
        CommandName::ClearSearch => Flow::Run(Command::ClearSearch {}),
        CommandName::OpenComposer => on_draft(state, false, |id| Command::OpenComposer { id }),
        CommandName::CloseComposer => Flow::Run(Command::CloseComposer {}),
        CommandName::FocusPane => ask(
            "Focus which pane?",
            AskKind::Panes,
            |_, choice| match choice {
                Choice::Pane(pane) => Flow::Run(Command::FocusPane { pane }),
                _ => Flow::Hidden,
            },
        ),
        CommandName::ToggleSidebar => Flow::Run(Command::ToggleSidebar {}),
        CommandName::GetView => Flow::Run(Command::GetView {}),
        CommandName::CompactStore => Flow::Run(Command::CompactStore {}),
        CommandName::ListApprovals => Flow::Run(Command::ListApprovals {}),
        CommandName::ApproveRequest => ask(
            "Approve which send request?",
            AskKind::Approvals,
            |_, choice| match choice {
                Choice::Approval(id) => Flow::Run(Command::ApproveRequest { id }),
                _ => Flow::Hidden,
            },
        ),
        CommandName::RejectRequest => ask(
            "Reject which send request?",
            AskKind::Approvals,
            |_, choice| match choice {
                Choice::Approval(id) => Flow::Run(Command::RejectRequest { id }),
                _ => Flow::Hidden,
            },
        ),
    }
}

/// Whether the palette offers `name` at all.
pub const fn offered(name: CommandName) -> bool {
    !matches!(name, CommandName::OpenPalette | CommandName::RestoreDraft)
}

/// One row of the palette.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// What choosing it does.
    pub target: Target,
    /// Main label.
    pub title: String,
    /// Dimmed second label (category, account, sender).
    pub detail: String,
    /// Shortcut hint.
    pub keys: Option<String>,
}

/// A run of rows under one caption ([`Palette::sections`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// The caption: `Recent`, a command category, `Mailboxes` or `Threads`.
    pub title: &'static str,
    /// The first row.
    pub start: usize,
    /// One past the last row.
    pub end: usize,
}

/// What a row stands for.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    /// A registry command.
    Command(CommandName),
    /// Go to a mailbox.
    Mailbox(MailboxId),
    /// Open a thread.
    Thread(ThreadId),
    /// Search for the typed text.
    Search(String),
    /// An answer to the current second step.
    Choice(Choice),
}

/// The first-step rows: available commands, mailboxes and recent threads.
pub fn root_rows(state: &AppState) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut specs = app_core::Command::specs();
    // Everyday commands first; the sort is stable, so the catalogue order holds within a group.
    specs.sort_by_key(|spec| match spec.category {
        Category::Mail => 0,
        Category::Compose => 1,
        Category::Navigation => 2,
        Category::Account => 3,
        Category::App => 4,
    });
    for spec in specs {
        let Some(name) = CommandName::from_name(spec.name) else {
            continue;
        };
        if offered(name) && (spec.available)(state) {
            rows.push(Row {
                target: Target::Command(name),
                title: spec.title.to_owned(),
                detail: format!("{:?}", spec.category),
                keys: crate::keymap::keys_for(&state.settings, name)
                    .into_iter()
                    .next(),
            });
        }
    }
    for mailbox in listing::mailboxes(state, None) {
        let account = state
            .accounts
            .get(&mailbox.account)
            .map(|a| a.display_name.clone())
            .unwrap_or_default();
        let current = current_account(state) == Some(mailbox.account);
        let keys = GO_TO
            .iter()
            .find(|(_, role)| current && mailbox.role == Some(*role))
            .map(|(letter, _)| format!("g {letter}"));
        rows.push(Row {
            target: Target::Mailbox(mailbox.id),
            title: format!("Go to {}", mailbox_name(mailbox)),
            detail: account,
            keys,
        });
    }
    for (title, detail, id) in recent_threads(state) {
        rows.push(Row {
            target: Target::Thread(id),
            title,
            detail,
            keys: None,
        });
    }
    rows
}

fn recent_threads(state: &AppState) -> Vec<(String, String, ThreadId)> {
    let mut threads: Vec<_> = state.threads.values().collect();
    threads.sort_by(|a, b| {
        b.last_date_ms
            .cmp(&a.last_date_ms)
            .then_with(|| a.id.cmp(&b.id))
    });
    threads
        .into_iter()
        .take(RECENT_THREADS)
        .map(|t| {
            let summary = ThreadSummary::of(state, t);
            let subject = if t.subject.trim().is_empty() {
                "(no subject)"
            } else {
                &t.subject
            };
            (subject.to_owned(), format::names(&summary.senders), t.id)
        })
        .collect()
}

/// The rows of a second step.
pub fn ask_rows(state: &AppState, kind: &AskKind, query: &str) -> Vec<Row> {
    let row = |choice: Choice, title: String, detail: String| Row {
        target: Target::Choice(choice),
        title,
        detail,
        keys: None,
    };
    match kind {
        AskKind::Mailboxes(account) => listing::mailboxes(state, *account)
            .into_iter()
            .map(|m| {
                let owner = state
                    .accounts
                    .get(&m.account)
                    .map(|a| a.display_name.clone())
                    .unwrap_or_default();
                row(Choice::Mailbox(m.id), mailbox_name(m), owner)
            })
            .collect(),
        AskKind::Threads => {
            let mut ids = targets(state);
            for id in listing::visible_threads(state) {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
            let mut rows: Vec<Row> = ids
                .into_iter()
                .filter_map(|id| state.threads.get(&id))
                .map(|t| {
                    let summary = ThreadSummary::of(state, t);
                    row(
                        Choice::Thread(t.id),
                        t.subject.clone(),
                        format::names(&summary.senders),
                    )
                })
                .collect();
            for (title, detail, id) in recent_threads(state) {
                if !rows
                    .iter()
                    .any(|r| r.target == Target::Choice(Choice::Thread(id)))
                {
                    rows.push(row(Choice::Thread(id), title, detail));
                }
            }
            rows
        }
        AskKind::Drafts { scheduled } => state
            .drafts
            .values()
            .filter(|d| {
                if *scheduled {
                    d.send_at.is_some() && !d.sending
                } else {
                    !d.sending
                }
            })
            .map(|d| {
                let to: Vec<&app_core::Addr> = d.to.iter().collect();
                let subject = if d.subject.trim().is_empty() {
                    "(no subject)".to_owned()
                } else {
                    d.subject.clone()
                };
                row(
                    Choice::Draft(d.id),
                    subject,
                    format!("to {}", format::names(&to)),
                )
            })
            .collect(),
        AskKind::Accounts => state
            .accounts
            .values()
            .map(|a| {
                row(
                    Choice::Account(a.id),
                    a.display_name.clone(),
                    a.email.clone(),
                )
            })
            .collect(),
        AskKind::Approvals => state
            .approvals
            .values()
            .map(|approval| {
                let draft = state.drafts.get(&approval.draft);
                let subject = draft
                    .map(|d| d.subject.trim())
                    .filter(|subject| !subject.is_empty())
                    .unwrap_or("(no subject)")
                    .to_owned();
                let to: Vec<&app_core::Addr> =
                    draft.map(|d| d.to.iter().collect()).unwrap_or_default();
                row(
                    Choice::Approval(approval.id),
                    subject,
                    format!("to {}", format::names(&to)),
                )
            })
            .collect(),
        AskKind::Ops => state
            .outbox
            .values()
            .map(|op| {
                let what = match &op.op {
                    app_core::RemoteOp::SetFlag {
                        flag,
                        value,
                        messages,
                    } => {
                        format!(
                            "{} {:?} on {} message(s)",
                            if *value { "set" } else { "clear" },
                            flag,
                            messages.len()
                        )
                    }
                    app_core::RemoteOp::Move { to, messages } => format!(
                        "move {} message(s) to {}",
                        messages.len(),
                        state
                            .mailboxes
                            .get(to)
                            .map_or_else(String::new, mailbox_name)
                    ),
                    app_core::RemoteOp::Copy { to, copies } => format!(
                        "copy {} message(s) to {}",
                        copies.len(),
                        state
                            .mailboxes
                            .get(to)
                            .map_or_else(String::new, mailbox_name)
                    ),
                };
                let detail = op
                    .last_error
                    .clone()
                    .unwrap_or_else(|| format!("{:?}", op.state).to_lowercase());
                row(Choice::Op(op.id), what, detail)
            })
            .collect(),
        AskKind::Settings => SettingKey::ALL
            .iter()
            .map(|key| {
                let (min, max) = key.range();
                row(
                    Choice::Setting(*key),
                    key.as_str().to_owned(),
                    format!("{} (range {min}–{max})", key.get(&state.settings)),
                )
            })
            .collect(),
        AskKind::Panes => Pane::ALL
            .iter()
            .filter(|pane| **pane == Pane::Sidebar || state.view.can_focus(**pane))
            .map(|pane| row(Choice::Pane(*pane), pane.as_str().to_owned(), String::new()))
            .collect(),
        AskKind::Attachments(draft) => state
            .drafts
            .get(draft)
            .map(|d| {
                d.attachments
                    .iter()
                    .zip(0_u32..)
                    .map(|(a, i)| row(Choice::Index(i), a.name.clone(), a.path.clone()))
                    .collect()
            })
            .unwrap_or_default(),
        AskKind::Text(hint) => {
            if query.trim().is_empty() {
                vec![Row {
                    target: Target::Choice(Choice::Text(String::new())),
                    title: "Type, then Enter".to_owned(),
                    detail: hint.clone(),
                    keys: None,
                }]
            } else {
                vec![row(
                    Choice::Text(query.to_owned()),
                    format!("Use “{query}”"),
                    hint.clone(),
                )]
            }
        }
        AskKind::Confirm(what) => vec![
            row(Choice::Yes, what.clone(), "cannot be undone".to_owned()),
            row(
                Choice::Text(String::new()),
                "Cancel".to_owned(),
                String::new(),
            ),
        ],
    }
}

/// `rows` that match `query` (in the title or the detail), best first: title matches before
/// detail-only matches (all rows, in order, for an empty query).
pub fn filter(rows: Vec<Row>, query: &str, matcher: &mut Matcher) -> Vec<Row> {
    let query = query.trim();
    if query.is_empty() {
        return rows;
    }
    let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
    let mut buf = Vec::new();
    let mut scored: Vec<(bool, u32, usize, Row)> = rows
        .into_iter()
        .enumerate()
        .filter_map(|(i, row)| {
            let haystack = format!("{} {}", row.title, row.detail);
            let score = pattern.score(Utf32Str::new(&haystack, &mut buf), matcher)?;
            let in_title = pattern
                .score(Utf32Str::new(&row.title, &mut buf), matcher)
                .is_some();
            Some((in_title, score, i, row))
        })
        .collect();
    // Rows whose title matches come before rows that match only through their detail ("Compose"
    // before "Send", whose category is Compose); equal scores: the shorter title is the closer
    // match ("Move to…" before "Move to Trash").
    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then(b.1.cmp(&a.1))
            .then(a.3.title.chars().count().cmp(&b.3.title.chars().count()))
            .then(a.2.cmp(&b.2))
    });
    scored.into_iter().map(|(_, _, _, row)| row).collect()
}

/// What choosing a row led to.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Chosen {
    /// Submit these as user commands, in order (the last one closes the palette).
    pub commands: Vec<Command>,
    /// Open the account wizard (adding an account needs a password form).
    pub wizard: bool,
    /// Open the password form for this account.
    pub password: Option<app_core::AccountId>,
}

/// The state of an open palette: the query, the highlighted row and the current second step.
/// A front-end draws it and feeds it the keys; closing is the engine's (`close_palette`).
pub struct Palette {
    query: String,
    selected: usize,
    step: Option<Ask>,
    matcher: Matcher,
    /// Commands chosen in this session: how often, and when (a count of choices).
    uses: Vec<Use>,
    /// Choices made so far.
    clock: u32,
}

/// How often and how lately a command was chosen in the palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Use {
    name: CommandName,
    count: u32,
    last: u32,
}

/// Commands remembered for frecency.
const MAX_USES: usize = 32;

impl Default for Palette {
    fn default() -> Self {
        Self {
            query: String::new(),
            selected: 0,
            step: None,
            matcher: Matcher::new(Config::DEFAULT),
            uses: Vec::new(),
            clock: 0,
        }
    }
}

impl fmt::Debug for Palette {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Palette")
            .field("query", &self.query)
            .field("selected", &self.selected)
            .field("step", &self.step)
            .finish_non_exhaustive()
    }
}

impl Palette {
    /// Starts at a second step (a key binding whose command needs an argument).
    pub fn open_at(&mut self, step: Ask) {
        self.step = Some(step);
        self.query.clear();
        self.selected = 0;
    }

    /// The typed text.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Replaces the typed text; the cursor goes back to the best match.
    pub fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
        self.selected = 0;
    }

    /// The typed text, for an input widget that edits it in place. Call [`Palette::reset_cursor`]
    /// when it changed.
    pub fn query_mut(&mut self) -> &mut String {
        &mut self.query
    }

    /// Moves the cursor back to the first row.
    pub fn reset_cursor(&mut self) {
        self.selected = 0;
    }

    /// The highlighted row's index (clamp it with [`Palette::clamp`] after the rows changed).
    pub const fn selected(&self) -> usize {
        self.selected
    }

    /// Keeps the cursor within `len` rows.
    pub fn clamp(&mut self, len: usize) {
        self.selected = self.selected.min(len.saturating_sub(1));
    }

    /// Moves the cursor by `step` rows within `len`.
    pub fn move_by(&mut self, step: isize, len: usize) {
        self.selected = self.selected.saturating_add_signed(step);
        self.clamp(len);
    }

    /// The current second step, if any.
    pub const fn step(&self) -> Option<&Ask> {
        self.step.as_ref()
    }

    /// The input's placeholder: the free-text hint, `Filter…` or the root prompt.
    pub fn hint(&self) -> String {
        match &self.step {
            Some(Ask {
                kind: AskKind::Text(hint),
                ..
            }) => hint.clone(),
            Some(_) => "Filter…".to_owned(),
            None => "Type a command or search…".to_owned(),
        }
    }

    /// Escape: leaves a second step for the command list. Returns `false` at the top level,
    /// where Escape closes the palette (`close_palette`).
    pub fn back(&mut self) -> bool {
        if self.step.is_none() {
            return false;
        }
        self.step = None;
        self.query.clear();
        self.selected = 0;
        true
    }

    /// How much `name` was used lately: its count, halved for every 8 choices since its last
    /// use (0 if never).
    fn frecency(&self, name: CommandName) -> u32 {
        self.uses.iter().find(|u| u.name == name).map_or(0, |u| {
            let age = self.clock.saturating_sub(u.last) / 8;
            (u.count * 8) >> age.min(31)
        })
    }

    /// Remembers that `name` was chosen.
    fn record(&mut self, name: CommandName) {
        self.clock = self.clock.saturating_add(1);
        let clock = self.clock;
        match self.uses.iter_mut().find(|u| u.name == name) {
            Some(used) => {
                used.count = used.count.saturating_add(1);
                used.last = clock;
            }
            None => {
                self.uses.push(Use {
                    name,
                    count: 1,
                    last: clock,
                });
                if self.uses.len() > MAX_USES {
                    // Forget the least recently used.
                    if let Some(oldest) = self
                        .uses
                        .iter()
                        .enumerate()
                        .min_by_key(|(_, u)| u.last)
                        .map(|(i, _)| i)
                    {
                        self.uses.remove(oldest);
                    }
                }
            }
        }
    }

    /// Puts the commands used lately first (stable among equals): with no query, by frecency;
    /// with one, among rows that match equally well (title or detail) by frecency first.
    fn by_frecency(&self, rows: Vec<Row>, query: &str) -> Vec<Row> {
        if self.uses.is_empty() {
            return rows;
        }
        let rank = |row: &Row| match row.target {
            Target::Command(name) => self.frecency(name),
            _ => 0,
        };
        let mut rows: Vec<(usize, Row)> = rows.into_iter().enumerate().collect();
        if query.trim().is_empty() {
            rows.sort_by(|a, b| rank(&b.1).cmp(&rank(&a.1)).then(a.0.cmp(&b.0)));
        } else {
            // Only among the best matches, so that a typed command name still wins.
            let best = rows.first().map(|(_, r)| r.title.to_lowercase());
            let query = query.trim().to_lowercase();
            let exact = best.is_some_and(|t| t.starts_with(&query));
            if !exact {
                let head = rows.len().min(5);
                rows[..head].sort_by(|a, b| rank(&b.1).cmp(&rank(&a.1)).then(a.0.cmp(&b.0)));
            }
        }
        rows.into_iter().map(|(_, row)| row).collect()
    }

    /// The sections of `rows` (from [`Self::rows`]) under captions, for an empty query on the
    /// first step: *Recent* (the commands used before, as frecency put them first), then one
    /// section per command category, *Mailboxes* and *Threads*. With a query or in a second
    /// step the rows are one ranked list, and this is empty. The rows keep their order, so the
    /// cursor still walks every row.
    pub fn sections(&self, rows: &[Row]) -> Vec<Section> {
        if self.step.is_some() || !self.query.trim().is_empty() {
            return Vec::new();
        }
        let key = |i: usize, row: &Row| -> &'static str {
            match &row.target {
                Target::Command(name) => {
                    if i < self.recent_head(rows) {
                        "Recent"
                    } else {
                        match name.spec().category {
                            Category::Mail => "Mail",
                            Category::Compose => "Compose",
                            Category::Navigation => "Navigation",
                            Category::Account => "Account",
                            Category::App => "App",
                        }
                    }
                }
                Target::Mailbox(_) => "Mailboxes",
                Target::Thread(_) => "Threads",
                Target::Search(_) | Target::Choice(_) => "",
            }
        };
        let mut out: Vec<Section> = Vec::new();
        for (i, row) in rows.iter().enumerate() {
            let title = key(i, row);
            match out.last_mut() {
                Some(last) if last.title == title => last.end = i + 1,
                _ => out.push(Section {
                    title,
                    start: i,
                    end: i + 1,
                }),
            }
        }
        out
    }

    /// How many leading rows frecency put first (commands used before).
    fn recent_head(&self, rows: &[Row]) -> usize {
        rows.iter()
            .take_while(|row| match row.target {
                Target::Command(name) => self.frecency(name) > 0,
                _ => false,
            })
            .count()
    }

    /// The highlighted row's help: the first sentence of its command's doc comment.
    pub fn help(rows: &[Row], selected: usize) -> Option<String> {
        let Target::Command(name) = rows.get(selected)?.target else {
            return None;
        };
        let text = name
            .spec()
            .description()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let end = text.find(". ").map_or(text.len(), |at| at + 1);
        let first: String = text[..end].chars().take(200).collect();
        (!first.is_empty()).then_some(first)
    }

    /// The rows shown for the current step and query.
    pub fn rows(&mut self, state: &AppState) -> Vec<Row> {
        match &self.step {
            Some(step) => {
                let rows = ask_rows(state, &step.kind, &self.query);
                if matches!(step.kind, AskKind::Text(_)) {
                    rows
                } else {
                    filter(rows, &self.query, &mut self.matcher)
                }
            }
            None => {
                let rows = filter(root_rows(state), &self.query, &mut self.matcher);
                let mut rows = self.by_frecency(rows, &self.query.clone());
                let query = self.query.trim();
                if !query.is_empty() {
                    rows.push(Row {
                        target: Target::Search(query.to_owned()),
                        title: format!("Search mail for “{query}”"),
                        detail: "Navigation".to_owned(),
                        keys: Some("/".to_owned()),
                    });
                }
                rows
            }
        }
    }

    /// Chooses `target`: the commands to submit, or the next step (kept here).
    pub fn choose(&mut self, state: &AppState, target: Target) -> Chosen {
        let flow = match target {
            Target::Command(name) => {
                self.record(name);
                flow(name, state)
            }
            Target::Mailbox(id) => Flow::Run(Command::OpenMailbox { id }),
            Target::Thread(id) => Flow::Run(Command::OpenThread { id }),
            Target::Search(query) => Flow::Run(Command::SetSearch { query }),
            Target::Choice(choice) => match &self.step {
                Some(step) => step.answer(state, choice),
                None => Flow::Hidden,
            },
        };
        if !matches!(flow, Flow::Ask(_)) {
            // The flow ended; the palette closes once `close_palette` is through.
            self.step = None;
            self.query.clear();
            self.selected = 0;
        }
        let close = Command::ClosePalette {};
        match flow {
            Flow::Run(cmd) => {
                let commands = if cmd == close {
                    vec![cmd]
                } else {
                    vec![cmd, close]
                };
                Chosen {
                    commands,
                    ..Chosen::default()
                }
            }
            Flow::Ask(next) => {
                self.step = Some(next);
                self.query.clear();
                self.selected = 0;
                Chosen::default()
            }
            Flow::Wizard => Chosen {
                commands: vec![close],
                wizard: true,
                ..Chosen::default()
            },
            Flow::Password(account) => Chosen {
                commands: vec![close],
                password: Some(account),
                ..Chosen::default()
            },
            // A cancelled or empty answer ends the flow.
            Flow::Hidden => Chosen {
                commands: vec![close],
                ..Chosen::default()
            },
        }
    }
}
