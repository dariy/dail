//! The command registry: one [`CommandSpec`] per [`Command`] variant.
//!
//! The palette, CLI help, MCP `tools/list` and the parity tests all read from here. Every `match`
//! below is exhaustive (no `_ =>` arm), so adding a `Command` variant fails to compile until its
//! spec and example exist.

use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
use serde::{Deserialize, Serialize};

use crate::account::{ProviderPreset, Secret};
use crate::command::{Command, CommandKind, SettingKey};
use crate::draft::Draft;
use crate::ids::{AccountId, ApprovalId, DraftId, MailboxId, MessageId, OpId, ThreadId};
use crate::mailbox::FolderKey;
use crate::message::Addr;
use crate::state::{AppState, Pane};

/// Groups commands in the palette and in CLI help.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    /// Reading and triaging mail.
    Mail,
    /// Moving around the UI.
    Navigation,
    /// Writing and sending mail.
    Compose,
    /// Account management.
    Account,
    /// Application-wide actions (undo, settings).
    App,
}

/// Whether agents may run a command (`ARCHITECTURE.md §9`, ADR 0002). [`crate::Engine::dispatch`]
/// checks it for every command a front-end submits; the commands `undo`/`redo` apply come from
/// the engine's own history and are not checked again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentAccess {
    /// Agents (MCP) run it like the user.
    Allowed,
    /// Agents run it, but their sends wait for the user's approval (unless the user started
    /// `dail mcp --allow-send`).
    Approval,
    /// Only the user (GUI, CLI, batch) may run it; agents get `origin_not_allowed`.
    UserOnly,
}

/// Static metadata of one command.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct CommandSpec {
    /// `snake_case` name; equals the serde tag of the `Command` variant.
    pub name: &'static str,
    /// Short human label for the palette.
    pub title: &'static str,
    /// Palette / help grouping.
    pub category: Category,
    /// What the command does to the world.
    pub kind: CommandKind,
    /// Whether agents may run it.
    pub agents: AgentAccess,
    /// Default key bindings of the GUI and the TUI. Each command with keys has at least one that
    /// a plain terminal delivers (`app-tui/tests/it/keys.rs`).
    pub default_keys: &'static [&'static str],
    /// Whether the command makes sense in the given state (palette and menu filtering).
    #[serde(skip)]
    pub available: fn(&AppState) -> bool,
}

impl CommandSpec {
    /// The agent-facing description (the variant's doc comment).
    pub fn description(&self) -> String {
        CommandName::from_name(self.name)
            .map(CommandName::description)
            .unwrap_or_default()
    }
}

/// Field-less mirror of [`Command`], used to enumerate the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CommandName {
    /// See [`Command::GetSettings`].
    GetSettings,
    /// See [`Command::SetSetting`].
    SetSetting,
    /// See [`Command::SetKeybinding`].
    SetKeybinding,
    /// See [`Command::SetKeyPreset`].
    SetKeyPreset,
    /// See [`Command::SetTheme`].
    SetTheme,
    /// See [`Command::OpenSettings`].
    OpenSettings,
    /// See [`Command::CloseSettings`].
    CloseSettings,
    /// See [`Command::Undo`].
    Undo,
    /// See [`Command::Redo`].
    Redo,
    /// See [`Command::OpenPalette`].
    OpenPalette,
    /// See [`Command::ClosePalette`].
    ClosePalette,
    /// See [`Command::AddAccount`].
    AddAccount,
    /// See [`Command::TestAccount`].
    TestAccount,
    /// See [`Command::DiscoverServers`].
    DiscoverServers,
    /// See [`Command::UpdateAccount`].
    UpdateAccount,
    /// See [`Command::SetPassword`].
    SetPassword,
    /// See [`Command::SetIdentity`].
    SetIdentity,
    /// See [`Command::RemoveIdentity`].
    RemoveIdentity,
    /// See [`Command::SetMailboxIdentity`].
    SetMailboxIdentity,
    /// See [`Command::SetDraftFrom`].
    SetDraftFrom,
    /// See [`Command::RemoveAccount`].
    RemoveAccount,
    /// See [`Command::SyncNow`].
    SyncNow,
    /// See [`Command::ListMailboxes`].
    ListMailboxes,
    /// See [`Command::SetSyncWindow`].
    SetSyncWindow,
    /// See [`Command::ListThreads`].
    ListThreads,
    /// See [`Command::OpenMailbox`].
    OpenMailbox,
    /// See [`Command::OpenUnifiedInbox`].
    OpenUnifiedInbox,
    /// See [`Command::SelectThreads`].
    SelectThreads,
    /// See [`Command::CheckThreads`].
    CheckThreads,
    /// See [`Command::UncheckThreads`].
    UncheckThreads,
    /// See [`Command::OpenThread`].
    OpenThread,
    /// See [`Command::CloseThread`].
    CloseThread,
    /// See [`Command::ExpandMessages`].
    ExpandMessages,
    /// See [`Command::CollapseMessages`].
    CollapseMessages,
    /// See [`Command::GetThread`].
    GetThread,
    /// See [`Command::GetMessage`].
    GetMessage,
    /// See [`Command::ShowImagesFrom`].
    ShowImagesFrom,
    /// See [`Command::HideImagesFrom`].
    HideImagesFrom,
    /// See [`Command::SaveAttachments`].
    SaveAttachments,
    /// See [`Command::MarkRead`].
    MarkRead,
    /// See [`Command::MarkUnread`].
    MarkUnread,
    /// See [`Command::Star`].
    Star,
    /// See [`Command::Unstar`].
    Unstar,
    /// See [`Command::ArchiveThreads`].
    ArchiveThreads,
    /// See [`Command::TrashThreads`].
    TrashThreads,
    /// See [`Command::MoveThreads`].
    MoveThreads,
    /// See [`Command::ListPendingOps`].
    ListPendingOps,
    /// See [`Command::RetryPendingOps`].
    RetryPendingOps,
    /// See [`Command::DiscardPendingOp`].
    DiscardPendingOp,
    /// See [`Command::CreateDraft`].
    CreateDraft,
    /// See [`Command::Reply`].
    Reply,
    /// See [`Command::ReplyAll`].
    ReplyAll,
    /// See [`Command::Forward`].
    Forward,
    /// See [`Command::UpdateDraft`].
    UpdateDraft,
    /// See [`Command::DiscardDraft`].
    DiscardDraft,
    /// See [`Command::RestoreDraft`].
    RestoreDraft,
    /// See [`Command::AttachFile`].
    AttachFile,
    /// See [`Command::RemoveAttachment`].
    RemoveAttachment,
    /// See [`Command::SendDraft`].
    SendDraft,
    /// See [`Command::CancelSend`].
    CancelSend,
    /// See [`Command::Search`].
    Search,
    /// See [`Command::ListContacts`].
    ListContacts,
    /// See [`Command::SetSearch`].
    SetSearch,
    /// See [`Command::ClearSearch`].
    ClearSearch,
    /// See [`Command::OpenComposer`].
    OpenComposer,
    /// See [`Command::CloseComposer`].
    CloseComposer,
    /// See [`Command::FocusPane`].
    FocusPane,
    /// See [`Command::FocusMailbox`].
    FocusMailbox,
    /// See [`Command::ClearFocus`].
    ClearFocus,
    /// See [`Command::CollapseFolders`].
    CollapseFolders,
    /// See [`Command::ExpandFolders`].
    ExpandFolders,
    /// See [`Command::ToggleSidebar`].
    ToggleSidebar,
    /// See [`Command::GetView`].
    GetView,
    /// See [`Command::CompactStore`].
    CompactStore,
    /// See [`Command::ListApprovals`].
    ListApprovals,
    /// See [`Command::ApproveRequest`].
    ApproveRequest,
    /// See [`Command::RejectRequest`].
    RejectRequest,
}

const fn always(_: &AppState) -> bool {
    true
}

fn has_accounts(state: &AppState) -> bool {
    !state.accounts.is_empty()
}

fn has_mailboxes(state: &AppState) -> bool {
    !state.mailboxes.is_empty()
}

fn has_threads(state: &AppState) -> bool {
    !state.threads.is_empty()
}

fn has_messages(state: &AppState) -> bool {
    !state.messages.is_empty()
}

fn has_ops(state: &AppState) -> bool {
    !state.outbox.is_empty()
}

fn has_drafts(state: &AppState) -> bool {
    !state.drafts.is_empty()
}

fn has_approvals(state: &AppState) -> bool {
    !state.approvals.is_empty()
}

fn has_scheduled_sends(state: &AppState) -> bool {
    state
        .drafts
        .values()
        .any(|d| d.send_at.is_some() && !d.sending)
}

fn has_attachments(state: &AppState) -> bool {
    state.drafts.values().any(|d| !d.attachments.is_empty())
}

/// The account ID used by examples.
const EXAMPLE_ACCOUNT: AccountId = AccountId::from_uuid(uuid::Uuid::from_u128(1));

/// The mailbox ID used by examples.
const EXAMPLE_MAILBOX: MailboxId = MailboxId::from_uuid(uuid::Uuid::from_u128(2));

/// The thread ID used by examples.
const EXAMPLE_THREAD: ThreadId = ThreadId::from_uuid(uuid::Uuid::from_u128(3));

/// The message ID used by examples.
const EXAMPLE_MESSAGE: MessageId = MessageId::from_uuid(uuid::Uuid::from_u128(4));

/// The pending-op ID used by examples.
const EXAMPLE_OP: OpId = OpId::from_uuid(uuid::Uuid::from_u128(5));

/// The draft ID used by examples.
const EXAMPLE_DRAFT: DraftId = DraftId::from_uuid(uuid::Uuid::from_u128(6));
const EXAMPLE_APPROVAL: ApprovalId = ApprovalId::from_uuid(uuid::Uuid::from_u128(7));

fn example_addr() -> Addr {
    Addr {
        name: Some("Bob".to_owned()),
        email: "bob@example.org".to_owned(),
    }
}

impl CommandName {
    /// Every command, in catalogue order. The `registry_is_complete` test checks this list
    /// against the JSON Schema of [`Command`].
    pub const ALL: &'static [Self] = &[
        Self::GetSettings,
        Self::SetSetting,
        Self::SetKeybinding,
        Self::SetKeyPreset,
        Self::SetTheme,
        // Before the other `Escape` commands: an open settings page closes first.
        Self::OpenSettings,
        Self::CloseSettings,
        Self::Undo,
        Self::Redo,
        Self::OpenPalette,
        Self::ClosePalette,
        Self::AddAccount,
        Self::TestAccount,
        Self::DiscoverServers,
        Self::UpdateAccount,
        Self::SetPassword,
        Self::SetIdentity,
        Self::RemoveIdentity,
        Self::SetMailboxIdentity,
        Self::SetDraftFrom,
        Self::RemoveAccount,
        Self::SyncNow,
        Self::ListMailboxes,
        Self::SetSyncWindow,
        Self::ListThreads,
        Self::OpenMailbox,
        Self::OpenUnifiedInbox,
        Self::SelectThreads,
        Self::CheckThreads,
        Self::UncheckThreads,
        Self::OpenThread,
        Self::CloseThread,
        Self::ExpandMessages,
        Self::CollapseMessages,
        Self::GetThread,
        Self::GetMessage,
        Self::ShowImagesFrom,
        Self::HideImagesFrom,
        Self::SaveAttachments,
        Self::MarkRead,
        Self::MarkUnread,
        Self::Star,
        Self::Unstar,
        Self::ArchiveThreads,
        Self::TrashThreads,
        Self::MoveThreads,
        Self::ListPendingOps,
        Self::RetryPendingOps,
        Self::DiscardPendingOp,
        Self::CreateDraft,
        Self::Reply,
        Self::ReplyAll,
        Self::Forward,
        Self::UpdateDraft,
        Self::DiscardDraft,
        Self::RestoreDraft,
        Self::AttachFile,
        Self::RemoveAttachment,
        Self::SendDraft,
        Self::CancelSend,
        Self::Search,
        Self::ListContacts,
        Self::SetSearch,
        Self::ClearSearch,
        Self::OpenComposer,
        Self::CloseComposer,
        Self::FocusPane,
        Self::FocusMailbox,
        Self::ClearFocus,
        Self::CollapseFolders,
        Self::ExpandFolders,
        Self::ToggleSidebar,
        Self::GetView,
        Self::CompactStore,
        Self::ListApprovals,
        Self::ApproveRequest,
        Self::RejectRequest,
    ];

    /// The `snake_case` name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GetSettings => "get_settings",
            Self::SetSetting => "set_setting",
            Self::SetKeybinding => "set_keybinding",
            Self::SetKeyPreset => "set_key_preset",
            Self::SetTheme => "set_theme",
            Self::OpenSettings => "open_settings",
            Self::CloseSettings => "close_settings",
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::OpenPalette => "open_palette",
            Self::ClosePalette => "close_palette",
            Self::AddAccount => "add_account",
            Self::TestAccount => "test_account",
            Self::DiscoverServers => "discover_servers",
            Self::UpdateAccount => "update_account",
            Self::SetPassword => "set_password",
            Self::SetIdentity => "set_identity",
            Self::RemoveIdentity => "remove_identity",
            Self::SetMailboxIdentity => "set_mailbox_identity",
            Self::SetDraftFrom => "set_draft_from",
            Self::RemoveAccount => "remove_account",
            Self::SyncNow => "sync_now",
            Self::ListMailboxes => "list_mailboxes",
            Self::SetSyncWindow => "set_sync_window",
            Self::ListThreads => "list_threads",
            Self::OpenMailbox => "open_mailbox",
            Self::OpenUnifiedInbox => "open_unified_inbox",
            Self::SelectThreads => "select_threads",
            Self::CheckThreads => "check_threads",
            Self::UncheckThreads => "uncheck_threads",
            Self::OpenThread => "open_thread",
            Self::CloseThread => "close_thread",
            Self::ExpandMessages => "expand_messages",
            Self::CollapseMessages => "collapse_messages",
            Self::GetThread => "get_thread",
            Self::GetMessage => "get_message",
            Self::ShowImagesFrom => "show_images_from",
            Self::HideImagesFrom => "hide_images_from",
            Self::SaveAttachments => "save_attachments",
            Self::MarkRead => "mark_read",
            Self::MarkUnread => "mark_unread",
            Self::Star => "star",
            Self::Unstar => "unstar",
            Self::ArchiveThreads => "archive_threads",
            Self::TrashThreads => "trash_threads",
            Self::MoveThreads => "move_threads",
            Self::ListPendingOps => "list_pending_ops",
            Self::RetryPendingOps => "retry_pending_ops",
            Self::DiscardPendingOp => "discard_pending_op",
            Self::CreateDraft => "create_draft",
            Self::Reply => "reply",
            Self::ReplyAll => "reply_all",
            Self::Forward => "forward",
            Self::UpdateDraft => "update_draft",
            Self::DiscardDraft => "discard_draft",
            Self::RestoreDraft => "restore_draft",
            Self::AttachFile => "attach_file",
            Self::RemoveAttachment => "remove_attachment",
            Self::SendDraft => "send_draft",
            Self::CancelSend => "cancel_send",
            Self::Search => "search",
            Self::ListContacts => "list_contacts",
            Self::SetSearch => "set_search",
            Self::ClearSearch => "clear_search",
            Self::OpenComposer => "open_composer",
            Self::CloseComposer => "close_composer",
            Self::FocusPane => "focus_pane",
            Self::FocusMailbox => "focus_mailbox",
            Self::ClearFocus => "clear_focus",
            Self::CollapseFolders => "collapse_folders",
            Self::ExpandFolders => "expand_folders",
            Self::ToggleSidebar => "toggle_sidebar",
            Self::GetView => "get_view",
            Self::CompactStore => "compact_store",
            Self::ListApprovals => "list_approvals",
            Self::ApproveRequest => "approve_request",
            Self::RejectRequest => "reject_request",
        }
    }

    /// Looks a command up by its `snake_case` name.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|n| n.as_str() == name)
    }

    /// The command's kind.
    pub const fn kind(self) -> CommandKind {
        match self {
            Self::GetView
            | Self::GetSettings
            | Self::TestAccount
            | Self::DiscoverServers
            | Self::ListMailboxes
            | Self::ListThreads
            | Self::GetThread
            | Self::GetMessage
            | Self::ListPendingOps
            | Self::ListApprovals
            | Self::Search
            | Self::ListContacts => CommandKind::Query,
            // `undo`/`redo`, `sync_now` and the pending-op commands change data but are never
            // recorded in history.
            Self::SetSetting
            | Self::SetKeybinding
            | Self::SetKeyPreset
            | Self::SetTheme
            | Self::Undo
            | Self::Redo
            | Self::AddAccount
            | Self::UpdateAccount
            | Self::SetPassword
            | Self::SetIdentity
            | Self::RemoveIdentity
            | Self::SetMailboxIdentity
            | Self::SetDraftFrom
            | Self::SyncNow
            | Self::SetSyncWindow
            | Self::CompactStore
            | Self::ShowImagesFrom
            | Self::HideImagesFrom
            | Self::SaveAttachments
            | Self::MarkRead
            | Self::MarkUnread
            | Self::Star
            | Self::Unstar
            | Self::ArchiveThreads
            | Self::TrashThreads
            | Self::MoveThreads
            | Self::RetryPendingOps
            | Self::DiscardPendingOp
            | Self::CreateDraft
            | Self::Reply
            | Self::ReplyAll
            | Self::Forward
            | Self::UpdateDraft
            | Self::DiscardDraft
            | Self::RestoreDraft
            | Self::AttachFile
            | Self::RemoveAttachment
            | Self::ApproveRequest
            | Self::RejectRequest
            | Self::CancelSend => CommandKind::Mutation,
            Self::SendDraft => CommandKind::Outbound,
            Self::OpenPalette
            | Self::ClosePalette
            | Self::OpenSettings
            | Self::CloseSettings
            | Self::OpenMailbox
            | Self::OpenUnifiedInbox
            | Self::SelectThreads
            | Self::CheckThreads
            | Self::UncheckThreads
            | Self::OpenThread
            | Self::CloseThread
            | Self::ExpandMessages
            | Self::CollapseMessages
            | Self::SetSearch
            | Self::ClearSearch
            | Self::OpenComposer
            | Self::CloseComposer
            | Self::FocusPane
            | Self::FocusMailbox
            | Self::ClearFocus
            | Self::CollapseFolders
            | Self::ExpandFolders
            | Self::ToggleSidebar => CommandKind::View,
            Self::RemoveAccount => CommandKind::Destructive,
        }
    }

    /// Whether agents may run the command. User-only are the commands an agent could use to move
    /// data or credentials off the machine without the user: account servers (the password goes
    /// to them, and so do saved drafts with their attachments), restoring a draft from arguments
    /// (a forged attachment check or header) and deciding approval requests. Also user-only:
    /// writing files (`save_attachments`), which could plant a file where it gets run.
    pub const fn agent_access(self) -> AgentAccess {
        match self {
            Self::AddAccount
            | Self::UpdateAccount
            | Self::SetPassword
            | Self::SetIdentity
            | Self::RemoveIdentity
            | Self::RestoreDraft
            | Self::SaveAttachments
            | Self::ApproveRequest
            | Self::RejectRequest => AgentAccess::UserOnly,
            Self::SendDraft => AgentAccess::Approval,
            Self::GetSettings
            | Self::SetSetting
            | Self::SetKeybinding
            | Self::SetKeyPreset
            | Self::SetTheme
            | Self::Undo
            | Self::Redo
            | Self::OpenPalette
            | Self::ClosePalette
            | Self::TestAccount
            | Self::DiscoverServers
            | Self::RemoveAccount
            | Self::SetMailboxIdentity
            | Self::SetDraftFrom
            | Self::SyncNow
            | Self::ListMailboxes
            | Self::SetSyncWindow
            | Self::ListThreads
            | Self::OpenMailbox
            | Self::OpenUnifiedInbox
            | Self::SelectThreads
            | Self::CheckThreads
            | Self::UncheckThreads
            | Self::OpenThread
            | Self::CloseThread
            | Self::ExpandMessages
            | Self::CollapseMessages
            | Self::GetThread
            | Self::GetMessage
            | Self::ShowImagesFrom
            | Self::HideImagesFrom
            | Self::MarkRead
            | Self::MarkUnread
            | Self::Star
            | Self::Unstar
            | Self::ArchiveThreads
            | Self::TrashThreads
            | Self::MoveThreads
            | Self::ListPendingOps
            | Self::RetryPendingOps
            | Self::DiscardPendingOp
            | Self::CreateDraft
            | Self::Reply
            | Self::ReplyAll
            | Self::Forward
            | Self::UpdateDraft
            | Self::DiscardDraft
            | Self::AttachFile
            | Self::RemoveAttachment
            | Self::CancelSend
            | Self::Search
            | Self::ListContacts
            | Self::SetSearch
            | Self::ClearSearch
            | Self::OpenComposer
            | Self::OpenSettings
            | Self::CloseSettings
            | Self::CloseComposer
            | Self::FocusPane
            | Self::FocusMailbox
            | Self::ClearFocus
            | Self::CollapseFolders
            | Self::ExpandFolders
            | Self::ToggleSidebar
            | Self::GetView
            | Self::CompactStore
            | Self::ListApprovals => AgentAccess::Allowed,
        }
    }

    /// The command's spec.
    pub fn spec(self) -> CommandSpec {
        let base = |title, category, default_keys| CommandSpec {
            name: self.as_str(),
            title,
            category,
            kind: self.kind(),
            agents: self.agent_access(),
            default_keys,
            available: always,
        };
        match self {
            Self::GetSettings => base("Show settings", Category::App, &[]),
            Self::SetSetting => base("Change setting", Category::App, &[]),
            Self::SetKeybinding => base("Change a command's keys", Category::App, &[]),
            Self::SetKeyPreset => base("Choose the key bindings (Gmail, Vim)", Category::App, &[]),
            Self::SetTheme => base("Choose the theme", Category::App, &[]),
            Self::OpenSettings => base("Settings", Category::App, &["Ctrl+,", "g ,"]),
            Self::CloseSettings => CommandSpec {
                available: |state| state.view.settings.is_some(),
                ..base("Close settings", Category::App, &["Escape"])
            },
            Self::Undo => base("Undo", Category::App, &["z", "Ctrl+Z"]),
            Self::Redo => base("Redo", Category::App, &["Ctrl+Shift+Z", "Shift+Z"]),
            Self::OpenPalette => CommandSpec {
                available: |state| !state.view.palette_open,
                ..base(
                    "Open command palette",
                    Category::Navigation,
                    &["Ctrl+K", "Ctrl+Shift+P", ":"],
                )
            },
            Self::ClosePalette => CommandSpec {
                available: |state| state.view.palette_open,
                ..base("Close command palette", Category::Navigation, &["Escape"])
            },
            Self::AddAccount => base("Add account", Category::Account, &[]),
            Self::DiscoverServers => base(
                "Find the mail servers of an address",
                Category::Account,
                &[],
            ),
            Self::TestAccount => CommandSpec {
                available: has_accounts,
                ..base("Test account connection", Category::Account, &[])
            },
            Self::UpdateAccount => CommandSpec {
                available: has_accounts,
                ..base("Edit account", Category::Account, &[])
            },
            Self::SetPassword => CommandSpec {
                available: has_accounts,
                ..base("Change account password", Category::Account, &[])
            },
            Self::RemoveAccount => CommandSpec {
                available: has_accounts,
                ..base("Remove account", Category::Account, &[])
            },
            Self::SetIdentity => CommandSpec {
                available: has_accounts,
                ..base("Add or edit a sending address", Category::Account, &[])
            },
            Self::RemoveIdentity => CommandSpec {
                available: |state| state.accounts.values().any(|a| !a.identities.is_empty()),
                ..base("Remove a sending address", Category::Account, &[])
            },
            Self::SetMailboxIdentity => CommandSpec {
                available: |state| !state.mailboxes.is_empty(),
                ..base(
                    "Send from an address for this mailbox",
                    Category::Account,
                    &[],
                )
            },
            Self::SetDraftFrom => CommandSpec {
                available: |state| !state.drafts.is_empty(),
                ..base("Send this draft from…", Category::Compose, &[])
            },
            Self::SyncNow => CommandSpec {
                available: has_accounts,
                ..base("Sync now", Category::Mail, &["F5", "Ctrl+R"])
            },
            Self::ListMailboxes => CommandSpec {
                available: has_accounts,
                ..base("List mailboxes", Category::Mail, &[])
            },
            Self::SetSyncWindow => CommandSpec {
                available: |state| !state.mailboxes.is_empty(),
                ..base("Set how much mail to keep offline", Category::Account, &[])
            },
            Self::ListThreads => CommandSpec {
                available: has_mailboxes,
                ..base("List threads", Category::Mail, &[])
            },
            Self::OpenUnifiedInbox => CommandSpec {
                available: |state| {
                    state
                        .mailboxes
                        .values()
                        .any(|m| m.role == Some(crate::mailbox::MailboxRole::Inbox))
                },
                ..base("Go to all inboxes", Category::Navigation, &["g u"])
            },
            Self::OpenMailbox => CommandSpec {
                available: has_mailboxes,
                ..base(
                    "Go to mailbox",
                    Category::Navigation,
                    &["g i", "g s", "g d", "g a", "g t"],
                )
            },
            Self::SelectThreads => CommandSpec {
                available: has_threads,
                ..base(
                    "Select threads",
                    Category::Navigation,
                    &["j", "k", "ArrowDown", "ArrowUp"],
                )
            },
            Self::CheckThreads => CommandSpec {
                available: has_threads,
                ..base(
                    "Select all conversations",
                    Category::Navigation,
                    &["x", "* a"],
                )
            },
            Self::UncheckThreads => CommandSpec {
                available: |state| !state.view.checked.is_empty(),
                ..base(
                    "Clear the checked conversations",
                    Category::Navigation,
                    &["* n"],
                )
            },
            Self::OpenThread => CommandSpec {
                available: has_threads,
                ..base("Open thread", Category::Navigation, &["Enter", "o"])
            },
            Self::CloseThread => CommandSpec {
                available: |state| state.view.thread.is_some(),
                ..base("Close thread", Category::Navigation, &["Escape"])
            },
            Self::ExpandMessages => CommandSpec {
                available: |state| state.view.thread.is_some(),
                ..base("Expand all messages", Category::Navigation, &[])
            },
            Self::CollapseMessages => CommandSpec {
                available: |state| state.view.thread.is_some(),
                ..base("Collapse all messages", Category::Navigation, &[])
            },
            Self::GetThread => CommandSpec {
                available: has_threads,
                ..base("Show thread", Category::Mail, &[])
            },
            Self::GetMessage => CommandSpec {
                available: has_messages,
                ..base("Show message", Category::Mail, &[])
            },
            Self::ShowImagesFrom => CommandSpec {
                available: has_messages,
                ..base("Show images from sender", Category::Mail, &[])
            },
            Self::HideImagesFrom => CommandSpec {
                available: |state| !state.settings.images_from.is_empty(),
                ..base("Hide images from sender", Category::Mail, &[])
            },
            Self::SaveAttachments => CommandSpec {
                available: has_messages,
                ..base("Save attachments", Category::Mail, &[])
            },
            Self::MarkRead => CommandSpec {
                available: has_threads,
                ..base("Mark as read", Category::Mail, &["u", "Shift+I"])
            },
            Self::MarkUnread => CommandSpec {
                available: has_threads,
                ..base("Mark as unread", Category::Mail, &["u", "Shift+U"])
            },
            Self::Star => CommandSpec {
                available: has_threads,
                ..base("Star", Category::Mail, &["s"])
            },
            Self::Unstar => CommandSpec {
                available: has_threads,
                ..base("Remove star", Category::Mail, &[])
            },
            Self::ArchiveThreads => CommandSpec {
                available: has_threads,
                ..base("Archive", Category::Mail, &["e"])
            },
            Self::TrashThreads => CommandSpec {
                available: has_threads,
                ..base("Move to Trash", Category::Mail, &["#"])
            },
            Self::MoveThreads => CommandSpec {
                available: has_threads,
                ..base("Move to…", Category::Mail, &["v"])
            },
            Self::ListPendingOps => CommandSpec {
                available: has_ops,
                ..base("Show pending changes", Category::Mail, &[])
            },
            Self::RetryPendingOps => CommandSpec {
                available: has_ops,
                ..base("Retry pending changes", Category::Mail, &[])
            },
            Self::DiscardPendingOp => CommandSpec {
                available: has_ops,
                ..base("Discard pending change", Category::Mail, &[])
            },
            Self::CreateDraft => CommandSpec {
                available: has_accounts,
                ..base("Compose", Category::Compose, &["c"])
            },
            Self::Reply => CommandSpec {
                available: has_messages,
                ..base("Reply", Category::Compose, &["r"])
            },
            Self::ReplyAll => CommandSpec {
                available: has_messages,
                ..base("Reply all", Category::Compose, &["a"])
            },
            Self::Forward => CommandSpec {
                available: has_messages,
                ..base("Forward", Category::Compose, &["f"])
            },
            Self::UpdateDraft => CommandSpec {
                available: has_drafts,
                ..base("Edit draft", Category::Compose, &[])
            },
            Self::DiscardDraft => CommandSpec {
                available: has_drafts,
                ..base("Discard draft", Category::Compose, &[])
            },
            // Only undo needs it; it is not offered in the palette.
            Self::RestoreDraft => CommandSpec {
                available: |_| false,
                ..base("Restore draft", Category::Compose, &[])
            },
            Self::AttachFile => CommandSpec {
                available: has_drafts,
                ..base("Attach file", Category::Compose, &[])
            },
            Self::RemoveAttachment => CommandSpec {
                available: has_attachments,
                ..base("Remove attachment", Category::Compose, &[])
            },
            Self::SendDraft => CommandSpec {
                available: has_drafts,
                ..base("Send", Category::Compose, &["Ctrl+Enter", "Ctrl+S"])
            },
            Self::CancelSend => CommandSpec {
                available: has_scheduled_sends,
                ..base("Cancel send", Category::Compose, &[])
            },
            Self::Search => CommandSpec {
                available: has_messages,
                ..base("Search mail", Category::Mail, &[])
            },
            Self::ListContacts => CommandSpec {
                available: has_messages,
                ..base("List contacts", Category::Compose, &[])
            },
            Self::SetSearch => CommandSpec {
                available: has_messages,
                ..base("Search", Category::Navigation, &["/"])
            },
            Self::ClearSearch => CommandSpec {
                available: |state| state.view.search.is_some(),
                ..base("Clear search", Category::Navigation, &["Escape"])
            },
            Self::OpenComposer => CommandSpec {
                available: has_drafts,
                ..base("Open draft", Category::Compose, &[])
            },
            Self::CloseComposer => CommandSpec {
                available: |state| state.view.composer.is_some(),
                ..base("Close composer", Category::Compose, &["Escape"])
            },
            Self::ExpandFolders => CommandSpec {
                available: has_mailboxes,
                ..base("Expand folders", Category::Navigation, &["ArrowRight"])
            },
            Self::CollapseFolders => CommandSpec {
                available: has_mailboxes,
                ..base("Collapse folders", Category::Navigation, &["ArrowLeft"])
            },
            Self::ClearFocus => CommandSpec {
                available: |state| state.view.focused.is_some(),
                ..base("Exit focus", Category::Navigation, &[])
            },
            Self::FocusMailbox => CommandSpec {
                available: has_mailboxes,
                ..base(
                    "Focus this mailbox",
                    Category::Navigation,
                    &["Ctrl+Shift+F", "g f"],
                )
            },
            Self::FocusPane => base("Focus pane", Category::Navigation, &["Tab", "Shift+Tab"]),
            Self::ToggleSidebar => base("Toggle sidebar", Category::Navigation, &["Ctrl+B", "["]),
            Self::GetView => base("Show view state", Category::Navigation, &[]),
            Self::CompactStore => base("Compact the local store", Category::App, &[]),
            Self::ListApprovals => CommandSpec {
                available: has_approvals,
                ..base("Show send requests", Category::Compose, &[])
            },
            Self::ApproveRequest => CommandSpec {
                available: has_approvals,
                ..base("Approve send request", Category::Compose, &[])
            },
            Self::RejectRequest => CommandSpec {
                available: has_approvals,
                ..base("Reject send request", Category::Compose, &[])
            },
        }
    }

    /// A valid example of the command, used by parity tests and generated docs.
    pub fn example(self) -> Command {
        match self {
            Self::GetSettings => Command::GetSettings {},
            Self::SetKeybinding => Command::SetKeybinding {
                command: "archive_threads".to_owned(),
                keys: Some(vec!["e".to_owned(), "y".to_owned()]),
            },
            Self::SetKeyPreset => Command::SetKeyPreset {
                preset: crate::state::KeyPreset::Vim,
            },
            Self::SetTheme => Command::SetTheme {
                mode: Some(crate::command::ThemeMode::Dark),
                light: None,
                dark: Some("dail-dark-hc".to_owned()),
            },
            Self::SetSetting => Command::SetSetting {
                key: SettingKey::PollSecs,
                value: serde_json::Value::from(300_u64),
            },
            Self::Undo => Command::Undo {},
            Self::Redo => Command::Redo {},
            Self::OpenPalette => Command::OpenPalette {},
            Self::ClosePalette => Command::ClosePalette {},
            Self::AddAccount => Command::AddAccount {
                preset: ProviderPreset::Gmail,
                email: "me@gmail.com".to_owned(),
                display_name: Some("Me".to_owned()),
                password: None,
                imap: None,
                smtp: None,
            },
            Self::DiscoverServers => Command::DiscoverServers {
                email: "me@example.org".to_owned(),
            },
            Self::TestAccount => Command::TestAccount {
                id: EXAMPLE_ACCOUNT,
            },
            Self::UpdateAccount => Command::UpdateAccount {
                id: EXAMPLE_ACCOUNT,
                display_name: Some("Work".to_owned()),
                imap: None,
                smtp: None,
            },
            Self::SetPassword => Command::SetPassword {
                id: EXAMPLE_ACCOUNT,
                // Serialized as `<redacted>`: the example round-trips through JSON as that.
                password: Secret::new(Secret::REDACTED),
            },
            Self::SetIdentity => Command::SetIdentity {
                account: EXAMPLE_ACCOUNT,
                email: "sales@example.org".to_owned(),
                name: Some("Sales".to_owned()),
                signature: Some("Example Ltd.".to_owned()),
            },
            Self::RemoveIdentity => Command::RemoveIdentity {
                account: EXAMPLE_ACCOUNT,
                email: "sales@example.org".to_owned(),
            },
            Self::SetMailboxIdentity => Command::SetMailboxIdentity {
                mailbox: EXAMPLE_MAILBOX,
                email: Some("sales@example.org".to_owned()),
            },
            Self::SetDraftFrom => Command::SetDraftFrom {
                id: EXAMPLE_DRAFT,
                email: "sales@example.org".to_owned(),
            },
            Self::RemoveAccount => Command::RemoveAccount {
                id: EXAMPLE_ACCOUNT,
                confirm: true,
            },
            Self::SyncNow => Command::SyncNow { account: None },
            Self::ListMailboxes => Command::ListMailboxes { account: None },
            Self::SetSyncWindow => Command::SetSyncWindow {
                mailbox: EXAMPLE_MAILBOX,
                window: Some(0),
            },
            Self::ListThreads => Command::ListThreads {
                mailbox: EXAMPLE_MAILBOX,
                offset: 0,
                limit: 50,
            },
            Self::OpenUnifiedInbox => Command::OpenUnifiedInbox {},
            Self::OpenMailbox => Command::OpenMailbox {
                id: EXAMPLE_MAILBOX,
            },
            Self::SelectThreads => Command::SelectThreads {
                ids: vec![EXAMPLE_THREAD],
            },
            Self::CheckThreads => Command::CheckThreads { ids: Vec::new() },
            Self::UncheckThreads => Command::UncheckThreads { ids: Vec::new() },
            Self::OpenThread => Command::OpenThread { id: EXAMPLE_THREAD },
            Self::CloseThread => Command::CloseThread {},
            Self::ExpandMessages => Command::ExpandMessages {
                ids: vec![EXAMPLE_MESSAGE],
            },
            Self::CollapseMessages => Command::CollapseMessages { ids: Vec::new() },
            Self::GetThread => Command::GetThread {
                id: EXAMPLE_THREAD,
                include_body: false,
                rich_body: false,
            },
            Self::GetMessage => Command::GetMessage {
                id: EXAMPLE_MESSAGE,
                include_body: true,
            },
            Self::ShowImagesFrom => Command::ShowImagesFrom {
                sender: example_addr().email,
            },
            Self::HideImagesFrom => Command::HideImagesFrom {
                sender: example_addr().email,
            },
            Self::SaveAttachments => Command::SaveAttachments {
                message: EXAMPLE_MESSAGE,
                parts: Vec::new(),
                dir: "/home/me/Downloads".to_owned(),
            },
            Self::MarkRead => Command::MarkRead {
                ids: vec![EXAMPLE_THREAD],
                messages: Vec::new(),
            },
            Self::MarkUnread => Command::MarkUnread {
                ids: Vec::new(),
                messages: vec![EXAMPLE_MESSAGE],
            },
            Self::Star => Command::Star {
                ids: vec![EXAMPLE_THREAD],
                messages: Vec::new(),
            },
            Self::Unstar => Command::Unstar {
                ids: vec![EXAMPLE_THREAD],
                messages: Vec::new(),
            },
            Self::ArchiveThreads => Command::ArchiveThreads {
                ids: vec![EXAMPLE_THREAD],
                messages: Vec::new(),
            },
            Self::TrashThreads => Command::TrashThreads {
                ids: vec![EXAMPLE_THREAD],
                messages: Vec::new(),
            },
            Self::MoveThreads => Command::MoveThreads {
                ids: vec![EXAMPLE_THREAD],
                messages: Vec::new(),
                to: EXAMPLE_MAILBOX,
            },
            Self::ListPendingOps => Command::ListPendingOps { account: None },
            Self::RetryPendingOps => Command::RetryPendingOps { id: None },
            Self::DiscardPendingOp => Command::DiscardPendingOp { id: EXAMPLE_OP },
            Self::CreateDraft => Command::CreateDraft {
                account: EXAMPLE_ACCOUNT,
                to: vec![example_addr()],
                cc: Vec::new(),
                bcc: Vec::new(),
                subject: "Lunch?".to_owned(),
                body: "Tomorrow at noon?".to_owned(),
            },
            Self::Reply => Command::Reply {
                message: EXAMPLE_MESSAGE,
                body: Some("Thanks!".to_owned()),
            },
            Self::ReplyAll => Command::ReplyAll {
                message: EXAMPLE_MESSAGE,
                body: None,
            },
            Self::Forward => Command::Forward {
                message: EXAMPLE_MESSAGE,
                body: Some("FYI".to_owned()),
            },
            Self::UpdateDraft => Command::UpdateDraft {
                id: EXAMPLE_DRAFT,
                to: Some(vec![example_addr()]),
                cc: None,
                bcc: None,
                subject: None,
                body: Some("Tomorrow at one?".to_owned()),
            },
            Self::DiscardDraft => Command::DiscardDraft { id: EXAMPLE_DRAFT },
            Self::AttachFile => Command::AttachFile {
                draft: EXAMPLE_DRAFT,
                path: "/home/me/q3.pdf".to_owned(),
                index: None,
            },
            Self::RemoveAttachment => Command::RemoveAttachment {
                draft: EXAMPLE_DRAFT,
                index: 0,
            },
            Self::SendDraft => Command::SendDraft { id: EXAMPLE_DRAFT },
            Self::CancelSend => Command::CancelSend { id: EXAMPLE_DRAFT },
            Self::ListContacts => Command::ListContacts {
                query: "bob".to_owned(),
                limit: 20,
            },
            Self::Search => Command::Search {
                query: "from:alice is:unread after:2024-01-01".to_owned(),
                account: None,
                offset: 0,
                limit: 50,
                server: true,
                order: crate::search::SearchOrder::default(),
            },
            Self::SetSearch => Command::SetSearch {
                query: "has:attachment from:bob".to_owned(),
            },
            Self::ClearSearch => Command::ClearSearch {},
            Self::OpenComposer => Command::OpenComposer { id: EXAMPLE_DRAFT },
            Self::CloseComposer => Command::CloseComposer {},
            Self::OpenSettings => Command::OpenSettings {
                section: Some(crate::state::SettingsSection::Keys),
            },
            Self::CloseSettings => Command::CloseSettings {},
            Self::ExpandFolders => Command::ExpandFolders { keys: Vec::new() },
            Self::CollapseFolders => Command::CollapseFolders {
                keys: vec![FolderKey {
                    account: EXAMPLE_ACCOUNT,
                    path: "Work".to_owned(),
                }],
            },
            Self::ClearFocus => Command::ClearFocus {},
            Self::FocusMailbox => Command::FocusMailbox {
                id: Some(EXAMPLE_MAILBOX),
                account: None,
            },
            Self::FocusPane => Command::FocusPane { pane: Pane::Reader },
            Self::ToggleSidebar => Command::ToggleSidebar {},
            Self::GetView => Command::GetView {},
            Self::CompactStore => Command::CompactStore {},
            Self::ListApprovals => Command::ListApprovals {},
            Self::ApproveRequest => Command::ApproveRequest {
                id: EXAMPLE_APPROVAL,
            },
            Self::RejectRequest => Command::RejectRequest {
                id: EXAMPLE_APPROVAL,
            },
            Self::RestoreDraft => Command::RestoreDraft {
                draft: Box::new(Draft {
                    id: EXAMPLE_DRAFT,
                    account: EXAMPLE_ACCOUNT,
                    reply_to: None,
                    message_id_hdr: format!("{EXAMPLE_DRAFT}@example.org"),
                    in_reply_to: None,
                    references: Vec::new(),
                    to: vec![example_addr()],
                    cc: Vec::new(),
                    bcc: Vec::new(),
                    subject: "Lunch?".to_owned(),
                    body_text: "Tomorrow at noon?".to_owned(),
                    quote: None,
                    attachments: Vec::new(),
                    created_ms: 0,
                    updated_ms: 0,
                    saved_ms: None,
                    save_error: None,
                    send_at: None,
                    sending: false,
                    send_error: None,
                    from: None,
                }),
            },
        }
    }

    /// The agent-facing description (the variant's doc comment, via the JSON Schema).
    pub fn description(self) -> String {
        variant_schema(self)
            .and_then(|variant| {
                variant
                    .get("description")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            })
            .unwrap_or_default()
    }

    /// JSON Schema of the command's `args` object, with all subschemas inlined. This is the MCP
    /// tool input schema.
    pub fn args_schema(self) -> serde_json::Value {
        variant_schema(self)
            .and_then(|variant| variant.pointer("/properties/args").cloned())
            .unwrap_or_else(|| serde_json::json!({ "type": "object" }))
    }
}

/// JSON Schema (draft 2020-12) of the whole [`Command`] enum.
pub fn command_schema() -> serde_json::Value {
    schema_value::<Command>(false)
}

/// JSON Schema (draft 2020-12) of [`crate::Event`].
pub fn event_schema() -> serde_json::Value {
    schema_value::<crate::Event>(false)
}

/// The `cmd` tag of each variant in the [`Command`] schema, in schema order.
pub fn schema_command_names() -> Vec<String> {
    variants(&schema_value::<Command>(true))
        .iter()
        .filter_map(variant_tag)
        .collect()
}

fn schema_value<T: JsonSchema>(inline: bool) -> serde_json::Value {
    let generator = SchemaSettings::draft2020_12()
        .with(|settings| settings.inline_subschemas = inline)
        .into_generator();
    let schema = generator.into_root_schema_for::<T>();
    // reason: a schema is plain JSON; `Null` is a harmless fallback that tests would catch.
    serde_json::to_value(schema).unwrap_or_default()
}

fn variants(schema: &serde_json::Value) -> Vec<serde_json::Value> {
    ["oneOf", "anyOf"]
        .iter()
        .find_map(|key| schema.get(*key).and_then(serde_json::Value::as_array))
        .cloned()
        .unwrap_or_default()
}

fn variant_tag(variant: &serde_json::Value) -> Option<String> {
    let tag = variant.pointer("/properties/cmd")?;
    tag.get("const")
        .or_else(|| tag.pointer("/enum/0"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

fn variant_schema(name: CommandName) -> Option<serde_json::Value> {
    variants(&schema_value::<Command>(true))
        .into_iter()
        .find(|variant| variant_tag(variant).as_deref() == Some(name.as_str()))
}
