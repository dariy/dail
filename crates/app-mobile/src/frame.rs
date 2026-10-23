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
    /// The command palette, while open (a sheet over the panes; Back leaves its step, then
    /// closes it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette: Option<PaletteSheet>,
    /// A query's result, while open (over the panes; Back closes it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<ResultView>,
    /// A tapped link's full target, before it opens (a sheet; Back closes it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<LinkSheet>,
    /// A received attachment shown in the app (over the panes; Back closes it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<Preview>,
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
    /// The server search's state, while a search is open: running, or its last failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
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
    /// The search words in the subject and the snippet, while a search is open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marks: Option<RowMarks>,
    /// TalkBack's name.
    pub a11y: String,
}

/// Text cut where the search words are, to draw them marked.
pub type Marked = Vec<Piece>;

/// A piece of [`Marked`] text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Piece {
    /// The text.
    pub text: String,
    /// Whether it is a search word.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hit: bool,
}

/// A row's subject and snippet, cut where the search words are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RowMarks {
    /// The subject.
    pub subject: Marked,
    /// The snippet.
    pub snippet: Marked,
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
    /// What can be done with it: archive, trash, star, read, *Move…*, reply.
    pub actions: Vec<Button>,
    /// The conversation above it in the list, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous: Option<Button>,
    /// The conversation below it in the list, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<Button>,
    /// Where it is in the list (`2 of 14`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<String>,
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
    /// The body as blocks, once downloaded (expanded messages only): an HTML body's document,
    /// or the plain text's paragraphs with its signature and quote folded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<Vec<DocBlock>>,
    /// Why the body is missing (`Loading…`, a download error).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Its attachments.
    pub attachments: Vec<Attachment>,
    /// *Save all*, with two attachments or more.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub save_all: Option<Button>,
    /// Whether the body's own pictures show, and the button that changes it for the sender,
    /// when the body has any (`show_images_from`; nothing remote ever loads).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub images: Option<ImagesBar>,
    /// *Links (N)*: every link target in the body, in full; a tap shows it before it opens.
    pub links: Vec<Button>,
}

/// A block of a message body (`app_core::document::Block`, cleaned, with action IDs).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DocBlock {
    /// A run of text; `\n` in a span is a line break.
    Paragraph {
        /// The text.
        spans: Vec<DocSpan>,
    },
    /// A heading, 1 to 6.
    Heading {
        /// 1 to 6.
        level: u8,
        /// The text.
        spans: Vec<DocSpan>,
    },
    /// A list.
    List {
        /// Numbered.
        ordered: bool,
        /// The first item's number.
        start: u32,
        /// The items, each a list of blocks.
        items: Vec<Vec<DocBlock>>,
    },
    /// Quoted blocks.
    Quote {
        /// The quoted blocks.
        blocks: Vec<DocBlock>,
    },
    /// Monospace text with its spacing kept, line by line.
    Preformatted {
        /// The lines.
        lines: Vec<String>,
    },
    /// A data table.
    Table {
        /// The rows, each a list of cells.
        rows: Vec<Vec<DocCell>>,
    },
    /// A horizontal rule.
    Rule,
    /// Quoted earlier messages or a signature, folded until tapped.
    Fold {
        /// `Quoted text` or `Signature`.
        label: String,
        /// Whether it is open.
        expanded: bool,
        /// Opens or folds it.
        toggle: ActionId,
        /// What it holds, while open.
        blocks: Vec<DocBlock>,
    },
}

/// A table cell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocCell {
    /// A header cell.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub header: bool,
    /// The text.
    pub spans: Vec<DocSpan>,
}

/// Text with one style.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocSpan {
    /// The text.
    pub text: String,
    /// Bold.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    /// Italic (also an image's alt text).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    /// Monospace.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub code: bool,
    /// Struck through.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub strike: bool,
    /// A search word, while a search is open.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hit: bool,
    /// A link: a tap shows its full target first (the text may say something else).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<ActionId>,
    /// A picture of the message, shown in place of `text` (its alt text) while the sender's
    /// pictures show.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<InlineImage>,
}

/// A link's full target, before it opens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LinkSheet {
    /// The target, in full.
    pub url: String,
    /// *Open* in the browser (a `mailto:` link opens a new draft instead).
    pub open: Button,
    /// *Copy* the target.
    pub copy: Button,
    /// Closes the sheet.
    pub close: Button,
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
    /// Shows it in the app ([`Preview`]).
    pub open: ActionId,
    /// Saves it through the system's file picker.
    pub save: ActionId,
}

/// The pictures of a message body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ImagesBar {
    /// Whether they show.
    pub shown: bool,
    /// `Pictures in this message are hidden.`, or that they show.
    pub text: String,
    /// *Show images from …* or *Hide images from …*.
    pub toggle: Button,
}

/// A picture carried in the message (`cid:`), drawn from the body's blob.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InlineImage {
    /// The body's blob key (`blob`'s `inline` request).
    pub blob: String,
    /// The part's Content-ID.
    pub cid: String,
}

/// How an attachment is shown in the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PreviewKind {
    /// Decoded by the host (`blob`'s `image` request: width, height, RGBA).
    Image,
    /// Cleaned text from the host (`blob`'s `text` request: UTF-8).
    Text,
    /// Not shown; it can be saved.
    None,
}

/// A received attachment, shown in the app. There is no "open with": other apps get it only
/// through *Save*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Preview {
    /// The file name.
    pub name: String,
    /// The MIME type and the size.
    pub detail: String,
    /// How to show it.
    pub kind: PreviewKind,
    /// The body's blob key.
    pub blob: String,
    /// The attachment's MIME part.
    pub part: usize,
    /// What to say instead of the content (no preview for this type).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// *Save*.
    pub save: Button,
    /// Closes it.
    pub close: Button,
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
    /// The attached files, in order.
    pub attachments: Vec<OutgoingFile>,
    /// What went wrong: a failed send (with its hint), a failed save.
    pub problems: Vec<String>,
    /// *Attach files* (`pick_files`) and *Discard*, while the draft is editable.
    pub actions: Vec<Button>,
}

/// A file attached to the open draft.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OutgoingFile {
    /// The file name recipients see.
    pub name: String,
    /// `checking…`, the size, or why it cannot be sent.
    pub detail: String,
    /// Whether the check failed (the draft cannot be sent with it).
    pub failed: bool,
    /// Removes it from the draft, while the draft is editable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remove: Option<ActionId>,
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

/// The command palette: the registry's commands, mailboxes and recent threads, then the
/// second steps of `app_front::palette`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PaletteSheet {
    /// The second step's question (`Move to which mailbox?`), if in one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// The input's placeholder.
    pub hint: String,
    /// The typed text (`text` events name the `query` field).
    pub query: TextValue,
    /// The rows, under captions (one untitled section with a query or in a step).
    pub sections: Vec<PaletteSection>,
    /// Leaves the step, else closes the palette.
    pub close: Button,
}

/// Rows of the palette under one caption.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PaletteSection {
    /// The caption (`Recent`, `Mail`, `Mailboxes`, …); empty for a ranked list.
    pub title: String,
    /// Its rows.
    pub rows: Vec<PaletteRow>,
}

/// One choice in the palette.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PaletteRow {
    /// The main label.
    pub title: String,
    /// The second label (a category, an account, the senders).
    pub detail: String,
    /// The key binding, for a hardware keyboard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys: Option<String>,
    /// Chooses it.
    pub action: ActionId,
}

/// A query's result as text, read-only. Email content in it is untrusted data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResultView {
    /// The command's title.
    pub title: String,
    /// Says that it is read-only and untrusted.
    pub note: String,
    /// The result (pretty JSON), line by line.
    pub lines: Vec<String>,
    /// Closes it.
    pub close: Button,
}
