//! The screen models: everything Compose draws, as data. Every string from mail has been through
//! `app_front::text` (`ARCHITECTURE.md §9`).

use app_core::{DraftId, MessageId, ThreadId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::event::Field;
use crate::theme::{About, FrameTheme};

/// An entry of the frame's action table. IDs only grow, so an ID from an older frame is never
/// taken for another action: the presenter ignores it.
pub type ActionId = u32;

/// One screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Frame {
    /// Counts the frames this presenter made.
    pub revision: u64,
    /// The window class and the panes on screen.
    pub layout: Layout,
    /// The top app bar.
    pub top_bar: TopBar,
    /// Accounts and mailboxes, while on screen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drawer: Option<Drawer>,
    /// The thread list, while on screen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list: Option<List>,
    /// The open thread, while on screen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reader: Option<Reader>,
    /// The open draft, while on screen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composer: Option<Composer>,
    /// The settings page, while open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<SettingsPage>,
    /// The bulk bar, while threads are checked (in place of the top bar's actions).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bulk: Option<Bulk>,
    /// The floating button (compose).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fab: Option<Button>,
    /// A problem to fix (a failed login, a failed sync).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub banner: Option<Banner>,
    /// A short message: a send counting down, an undoable action, a rejection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snackbar: Option<Snackbar>,
    /// The theme, in the first frame and whenever it changes (the settings, the system's dark
    /// mode); the host keeps the last one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<FrameTheme>,
    /// The About screen, while open (over everything else; Back closes it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub about: Option<About>,
    /// Whether system Back is taken (send `back`); otherwise Android handles it.
    pub back: bool,
    /// Whether to send `tick` every second (a countdown or an age is shown).
    pub wants_tick: bool,
}

/// Material's window size classes, by width.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Class {
    /// Under 600 dp: one pane.
    #[default]
    Compact,
    /// 600–839 dp: the list and the detail.
    Medium,
    /// 840 dp and wider: the drawer, the list and the detail.
    Expanded,
}

impl Class {
    /// The class of a window `width_dp` wide.
    pub const fn of(width_dp: u32) -> Self {
        if width_dp < 600 {
            Self::Compact
        } else if width_dp < 840 {
            Self::Medium
        } else {
            Self::Expanded
        }
    }
}

/// What a pane shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PaneKind {
    /// Accounts and mailboxes.
    Drawer,
    /// The thread list.
    List,
    /// The open thread.
    Reader,
    /// The open draft.
    Composer,
    /// The settings page.
    Settings,
}

/// The window class and the panes, left to right.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Layout {
    /// The window class.
    pub class: Class,
    /// The panes on screen, left to right.
    pub panes: Vec<PaneKind>,
}

/// Something to tap: an icon or a text button.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Button {
    /// The icon's name (`menu`, `back`, `search`, `edit`, `archive`, …), if it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// The label (a tooltip and TalkBack's name for an icon).
    pub label: String,
    /// What a tap does.
    pub action: ActionId,
}

/// The top app bar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TopBar {
    /// Where the user is.
    pub title: String,
    /// The sync state (`synced 2m ago`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    /// The navigation button: the drawer, or back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub navigation: Option<Button>,
    /// The buttons on the right.
    pub actions: Vec<Button>,
}

/// Accounts and their mailboxes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Drawer {
    /// A modal drawer over the panes of `layout` (compact and medium windows); otherwise it is
    /// the first pane.
    pub modal: bool,
    /// A navigation rail (icons only) rather than the full drawer (`toggle_sidebar`).
    pub rail: bool,
    /// Every account's inbox, with two accounts or more.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all_inboxes: Option<DrawerItem>,
    /// One section per account.
    pub accounts: Vec<DrawerAccount>,
    /// Below the accounts: *Settings*, *Add account*.
    pub footer: Vec<DrawerItem>,
}

/// An account in the drawer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DrawerAccount {
    /// The account's name.
    pub name: String,
    /// Its address.
    pub email: String,
    /// Its colour: an index into the theme's `tags`, with two accounts or more.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<usize>,
    /// Its mailboxes, in the sidebar's order.
    pub mailboxes: Vec<DrawerItem>,
}

/// A mailbox in the drawer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DrawerItem {
    /// The mailbox's name.
    pub label: String,
    /// The icon's name (`inbox`, `send`, `folder`, …).
    pub icon: String,
    /// Unread messages.
    pub unread: u32,
    /// Whether the list shows it.
    pub selected: bool,
    /// Opens it.
    pub action: ActionId,
    /// TalkBack's name.
    pub a11y: String,
}

/// Row heights (`list_density`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Density {
    /// Two lines.
    Cozy,
    /// One line.
    Compact,
    /// Three lines, with the snippet.
    Preview,
}

/// The thread list: a window of rows around the visible ones.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct List {
    /// The mailbox or the search.
    pub title: String,
    /// Rows in the whole list (`LazyColumn`'s item count).
    pub total: usize,
    /// The index of `rows[0]`; rows outside the window are placeholders.
    pub offset: usize,
    /// Row heights.
    pub density: Density,
    /// Checked threads (a bulk bar shows while any are).
    pub checked: usize,
    /// A sync is running (pull to refresh shows its spinner).
    pub refreshing: bool,
    /// The rows of the window.
    pub rows: Vec<Row>,
    /// What to say when the list is empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empty: Option<String>,
}

/// One thread in the list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Row {
    /// The thread (the item key).
    pub thread: ThreadId,
    /// A tap opens it (checks or unchecks it while threads are checked).
    pub action: ActionId,
    /// A long press checks it.
    pub long_press: ActionId,
    /// Swiped towards the start edge: archive.
    pub swipe_start: ActionId,
    /// Swiped towards the end edge: trash.
    pub swipe_end: ActionId,
    /// The senders.
    pub sender: String,
    /// The subject.
    pub subject: String,
    /// The start of the newest message.
    pub snippet: String,
    /// The newest message's date, short.
    pub date: String,
    /// Whether a message is unread.
    pub unread: bool,
    /// Whether a message is starred.
    pub starred: bool,
    /// Whether a message has attachments.
    pub attachments: bool,
    /// Messages in the thread.
    pub messages: usize,
    /// Its account's colour (an index into the theme's `tags`), with two accounts or more.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<usize>,
    /// Whether it is the open or selected thread.
    pub selected: bool,
    /// Whether it is checked.
    pub checked: bool,
    /// TalkBack's name.
    pub a11y: String,
}

/// What can be done with the checked threads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Bulk {
    /// `3 selected`.
    pub title: String,
    /// Archive, trash, read, unread, star.
    pub actions: Vec<Button>,
    /// Unchecks every thread.
    pub close: Button,
}

/// The open thread.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Reader {
    /// The thread.
    pub thread: ThreadId,
    /// Its subject.
    pub subject: String,
    /// What can be done with it.
    pub actions: Vec<Button>,
    /// Its messages, oldest first.
    pub messages: Vec<ReaderMessage>,
}

/// A message of the open thread.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReaderMessage {
    /// The message.
    pub message: MessageId,
    /// The sender, in full (`Name <address>`).
    pub from: String,
    /// The recipients (`To` and `Cc`), in full.
    pub to: String,
    /// The date, short.
    pub date: String,
    /// Whether the body shows.
    pub expanded: bool,
    /// Expands or collapses it.
    pub toggle: ActionId,
    /// The start of the text, for a collapsed message.
    pub snippet: String,
    /// The body's lines, once downloaded (expanded messages only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<Vec<String>>,
    /// Why the body is missing (`Loading…`, a download error).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Its attachments.
    pub attachments: Vec<Attachment>,
    /// The link targets in the body, in full.
    pub links: Vec<String>,
}

/// An attachment of a received message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Attachment {
    /// The MIME part.
    pub part: usize,
    /// The file name.
    pub name: String,
    /// The MIME type.
    pub mime: String,
    /// The size, for people (`12 KB`).
    pub size: String,
}

/// The settings page (its sections arrive with Wave 4 A6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SettingsPage {
    /// The open section (`appearance`, `keys`, `general`, `accounts`).
    pub section: String,
    /// What the page says.
    pub text: String,
}

/// A text field's content. The host reloads its field only when `version` changes, so a frame
/// never overwrites what the IME is composing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TextValue {
    /// The text.
    pub text: String,
    /// Changes whenever the presenter replaces the text (another draft, an edit from elsewhere,
    /// addresses taken from the typed text).
    pub version: u64,
}

/// An address in a recipient field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Chip {
    /// The address, in full (`Name <address>`).
    pub label: String,
    /// Removes it from the field.
    pub remove: ActionId,
}

/// `To`, `Cc` or `Bcc`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Recipients {
    /// Which field (`text` and `blur` events name it).
    pub field: Field,
    /// `To`, `Cc` or `Bcc`.
    pub label: String,
    /// Its addresses.
    pub chips: Vec<Chip>,
    /// What is typed and not yet an address.
    pub input: TextValue,
    /// Contacts matching the typed text; a tap adds one.
    pub suggestions: Vec<Button>,
}

/// One of the addresses a draft can be sent from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Choice {
    /// The address.
    pub label: String,
    /// Whether it is the draft's.
    pub selected: bool,
    /// Chooses it (`set_draft_from`).
    pub action: ActionId,
}

/// The original below a reply or a forward: read-only, folded until tapped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct QuoteFold {
    /// What the folded quote says (`Quoted message`, `Forwarded message`).
    pub label: String,
    /// The attribution or the forward header, line by line, while expanded.
    pub header: Vec<String>,
    /// Whether it is expanded.
    pub expanded: bool,
    /// Expands or folds it.
    pub toggle: ActionId,
}

/// The open draft, editable unless a send is scheduled or under way.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Composer {
    /// The draft.
    pub draft: DraftId,
    /// `New message`, `Reply` or `Forward`.
    pub title: String,
    /// `saved`, `saving…`, `sending in 7 s`, …
    pub status: String,
    /// Whether the fields take typing (not while a send is scheduled or leaving).
    pub editable: bool,
    /// The sending address.
    pub from: String,
    /// The account's addresses, with two or more (a menu on `from`).
    pub from_choices: Vec<Choice>,
    /// `To`, `Cc` and `Bcc`.
    pub recipients: Vec<Recipients>,
    /// The subject.
    pub subject: TextValue,
    /// The body (the quote stays below it).
    pub body: TextValue,
    /// The original, for a reply or a forward.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote: Option<QuoteFold>,
    /// What went wrong: a failed send (with its hint), a failed save.
    pub problems: Vec<String>,
    /// *Discard*.
    pub actions: Vec<Button>,
}

/// A problem and the action that may fix it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Banner {
    /// What went wrong.
    pub text: String,
    /// What it probably means and what to do.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// The fix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<Button>,
}

/// A short message at the bottom.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Snackbar {
    /// The message.
    pub text: String,
    /// Its button (Undo).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<Button>,
}
