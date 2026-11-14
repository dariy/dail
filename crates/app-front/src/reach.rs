//! The reach table (PAR-2): how a user reaches each registry command on each front end, so
//! that a command that exists on the desktop and is missing on the phone shows up in review.
//!
//! [`reach`] is an exhaustive `match` over [`CommandName`] with no catch-all arm: a new command
//! does not compile until it has a decision for every [`FrontEnd`]. The table describes the code
//! as it is, not as it should be; `tests/it/reach.rs` pins its gaps (every `Palette` and `Never`
//! entry per front end) and its `Flow` entries and enforces two rules: on touch no `Mutation` of category *Mail* is
//! `Palette` (the known exceptions are listed there with the roadmap item that closes each), and
//! nothing is `Never` unless an ADR says so.
//!
//! How the values are chosen:
//! - [`Reach::Control`]: a visible control runs the command: a button, a menu entry, a chip, a
//!   settings row, a form's submit button (the account wizard, the password form), a drag to a
//!   folder, a notification action, a visible back or close button (on touch the top bar's
//!   back arrow; the system Back gesture does not count, iOS has none).
//! - Keys. On the TUI a default key (`CommandSpec.default_keys`, or a key that toggles to the
//!   command, like `Shift+P` to `unpin_threads`) counts as `Control`, because keys are the TUI's
//!   controls, even when the command then asks for an argument in the palette. On the GUI a
//!   key alone does not count: the GUI is also used with a mouse, so it needs a visible control
//!   too. On touch keys never count (a hardware keyboard is optional).
//! - [`Reach::Flow`]: a visible control opens the command's palette step (a confirmation, a
//!   picker, a typed answer), like the folder menu's *Rename…*. A control that sometimes runs at
//!   once and sometimes asks first (`mark_mailbox_read`, which asks above 50 unread) is `Flow`.
//! - [`Reach::Palette`]: only through the palette (the parity walks prove it reaches every
//!   command it offers).
//! - [`Reach::Never`]: the front end cannot do it, by an ADR's decision.
//!
//! `restore_draft` is not offered in the palette: every front end reaches it through *Undo*
//! after `discard_draft`, which applies it, so it is `Control` where *Undo* is.

use app_core::CommandName;

/// A front end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrontEnd {
    /// The desktop GUI (`app-gui`).
    Gui,
    /// The terminal UI (`app-tui`).
    Tui,
    /// The touch presenter (`app-mobile`), for Android and iOS.
    Touch,
}

impl FrontEnd {
    /// Every front end.
    pub const ALL: &[Self] = &[Self::Gui, Self::Tui, Self::Touch];
}

/// How a user reaches a command on a front end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reach {
    /// A visible control runs it (on the TUI: a default key).
    Control,
    /// A visible control opens its palette step.
    Flow,
    /// Only through the palette.
    Palette,
    /// Not on this front end, by an ADR.
    Never,
}

/// One row of the table: the reach on the GUI, the TUI and touch.
struct Row(Reach, Reach, Reach);

/// How a user reaches `name` on `front`.
pub const fn reach(name: CommandName, front: FrontEnd) -> Reach {
    let Row(gui, tui, touch) = row(name);
    match front {
        FrontEnd::Gui => gui,
        FrontEnd::Tui => tui,
        FrontEnd::Touch => touch,
    }
}

const fn row(name: CommandName) -> Row {
    use CommandName as N;
    use Reach::{Control as C, Flow as F, Palette as P};
    match name {
        // Everywhere a control: navigation, the reader, the composer, triage, settings basics.
        N::OpenSettings
        | N::CloseSettings
        | N::Undo
        | N::OpenPalette
        | N::ClosePalette
        | N::AddAccount
        | N::TestAccount
        | N::DiscoverServers
        | N::SyncNow
        | N::PushSettings
        | N::DeleteRule
        | N::EnableRule
        | N::ApplyRules
        | N::OpenMailbox
        | N::OpenUnifiedInbox
        | N::CheckThreads
        | N::UncheckThreads
        | N::OpenThread
        | N::CloseThread
        | N::MarkRead
        | N::MarkUnread
        | N::Star
        | N::Unstar
        | N::PinThreads
        | N::UnpinThreads
        | N::ArchiveThreads
        | N::TrashThreads
        | N::MarkJunk
        | N::MarkNotJunk
        | N::CreateDraft
        | N::Reply
        | N::ReplyAll
        | N::Forward
        | N::UpdateDraft
        | N::DiscardDraft
        | N::EditDraft
        | N::RestoreDraft
        | N::SendDraft
        | N::OpenComposer
        | N::CloseComposer
        | N::CollapseFolders
        | N::ExpandFolders
        | N::ToggleSidebar
        | N::SkipUpdateVersion => Row(C, C, C),
        // Only through the palette everywhere: queries (the front ends show their data without
        // running them) and the forms PAR-5 and PAR-6 add.
        N::GetSettings
        | N::UpdateAccount
        | N::RemoveIdentity
        | N::SetMailboxIdentity
        | N::ListRules
        | N::ListMailboxes
        | N::ListThreads
        | N::GetThread
        | N::GetMessage
        | N::ListPendingOps
        | N::DiscardPendingOp
        | N::Search
        | N::ListContacts
        | N::GetView
        | N::CompactStore
        | N::ListApprovals => Row(P, P, P),
        // Settings rows on the GUI and touch; on the TUI a settings row opens a palette step.
        N::SetSetting | N::SetKeyPreset | N::SetTheme | N::SetUpdatePolicy => Row(C, F, C),
        N::SetKeybinding | N::SaveRule => Row(F, F, F),
        N::Redo => Row(P, C, P),
        // The GUI's password form (account settings, problems); the TUI's only through the
        // palette.
        N::SetPassword => Row(C, P, C),
        N::SetIdentity => Row(F, P, P),
        // The GUI's folder menu and list buttons; on touch the drawer's folder sheet, after a
        // long press on the folder (TCH-4).
        N::CreateFolder
        | N::RenameFolder
        | N::DeleteFolder
        | N::EmptyTrash
        | N::EmptyJunk
        | N::SetSyncWindow
        | N::MarkMailboxRead => Row(F, P, F),
        N::PinMailbox | N::UnpinMailbox | N::HideMailbox => Row(C, P, C),
        // The mobile drawer leaves hidden folders out and has no *Show hidden* toggle, so a
        // hidden folder's sheet (and its *Show folder*) never opens on touch.
        N::ShowMailbox => Row(C, P, P),
        N::SetListFilter | N::RetryPendingOps | N::ClearFocus => Row(C, P, P),
        N::FocusMailbox => Row(C, C, C),
        N::RemoveAccount => Row(F, P, F),
        N::PullSettings => Row(P, C, P),
        N::SetSettingsAccount => Row(F, F, P),
        N::SelectThreads => Row(P, C, P),
        N::FocusPane => Row(P, C, C),
        // The reader's and composer's controls the TUI does not draw.
        N::ExpandMessages
        | N::CollapseMessages
        | N::ShowImagesFrom
        | N::HideImagesFrom
        | N::SetDraftFrom
        | N::AttachFile
        | N::RemoveAttachment
        | N::CancelSend => Row(C, P, C),
        // Touch saves attachments through the host, and the command asks for a path (PAR-4).
        N::SaveAttachments => Row(C, P, P),
        // A drag to a folder on the GUI; *Move…* opens the folder picker on touch.
        N::MoveThreads => Row(C, C, F),
        N::SetSearch => Row(F, C, F),
        // The desktop's update panel; touch shows only a banner with *Skip*.
        N::CheckForUpdates | N::DownloadUpdate => Row(C, C, P),
        N::InstallUpdate => Row(F, F, P),
        // No approvals screen on touch (ADR 0009), but the palette offers them.
        N::ApproveRequest | N::RejectRequest => Row(C, C, P),
        // Touch has no visible control for it (the back arrow shows only over other panes).
        N::ClearSearch => Row(C, C, P),
    }
}
