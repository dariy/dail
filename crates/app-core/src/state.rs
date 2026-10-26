//! `AppState`: the single, serializable source of truth. Collections (accounts, mailboxes,
//! messages, threads, drafts, outbox, approvals) are added by the milestones that introduce them
//! (`ARCHITECTURE.md §4.2`).

use std::collections::{BTreeMap, BTreeSet};

use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::account::Account;
use crate::approval::ApprovalRequest;
use crate::draft::Draft;
use crate::ids::{AccountId, ApprovalId, DraftId, MailboxId, MessageId, OpId, ThreadId};
use crate::mailbox::{FolderKey, Mailbox};
use crate::message::{Envelope, Thread};
use crate::ops::PendingOp;

/// The whole application state. `Clone + Serialize`, no interior mutability.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AppState {
    /// Configured mail accounts, in the order they were added.
    pub accounts: IndexMap<AccountId, Account>,
    /// Mailboxes of all accounts, as last listed by their servers.
    pub mailboxes: IndexMap<MailboxId, Mailbox>,
    /// Cached message envelopes of all mailboxes (metadata only; bodies are blobs in the store).
    pub messages: IndexMap<MessageId, Envelope>,
    /// Conversations over `messages`.
    pub threads: IndexMap<ThreadId, Thread>,
    /// Messages being written, oldest first.
    #[serde(default)]
    pub drafts: IndexMap<DraftId, Draft>,
    /// Local changes not yet applied on the server, oldest first.
    #[serde(default)]
    pub outbox: IndexMap<OpId, PendingOp>,
    /// Agents' requests to send a draft, waiting for the user, oldest first.
    #[serde(default)]
    pub approvals: IndexMap<ApprovalId, ApprovalRequest>,
    /// What the user is looking at. Changed only by `View` commands.
    pub view: ViewState,
    /// User preferences.
    pub settings: Settings,
    /// Runtime health that must be visible to the user and to agents.
    pub status: StatusState,
    /// Counts the engine's inputs, so that a front-end can keep what it derived from a state
    /// until the next one (`listing` projections are O(n) per call). Not part of the state's
    /// value: never serialized, and equal in every comparison.
    #[serde(skip)]
    #[schemars(skip)]
    pub revision: Revision,
}

/// The number of inputs the engine had applied when a state was taken (see
/// [`AppState::revision`]). Two revisions always compare equal, so that states compare by
/// their contents.
#[derive(Debug, Clone, Copy, Default)]
pub struct Revision(u64);

impl Revision {
    /// The count.
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Counts one more input.
    pub const fn bump(&mut self) {
        self.0 = self.0.wrapping_add(1);
    }
}

impl PartialEq for Revision {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for Revision {}

/// Navigation and selection state.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ViewState {
    /// Whether the command palette is open.
    pub palette_open: bool,
    /// The mailbox shown in the thread list (`open_mailbox`).
    pub mailbox: Option<MailboxId>,
    /// The thread list shows every account's inbox (`open_unified_inbox`) instead of `mailbox`.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub unified: bool,
    /// Selected threads, in selection order (`select_threads`).
    pub selected: Vec<ThreadId>,
    /// Checked threads, in the order they were checked (`check_threads`): the GUI and the TUI
    /// apply triage to them, if there are any, instead of the selection. Cleared when the list
    /// shows another mailbox or search.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub checked: Vec<ThreadId>,
    /// The thread shown in the reader (`open_thread`).
    pub thread: Option<ThreadId>,
    /// Messages of the open thread the user expanded (`true`) or collapsed (`false`) in the
    /// reader (`expand_messages`, `collapse_messages`). The others follow the default: unread
    /// messages and the newest one are expanded (`listing::is_expanded`). Cleared when another
    /// thread opens.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub expanded: BTreeMap<MessageId, bool>,
    /// The search whose results the thread list shows instead of `mailbox` (`set_search`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
    /// The draft shown in the composer (`open_composer`).
    pub composer: Option<DraftId>,
    /// The pane that receives keyboard input (`focus_pane`).
    pub focus: Pane,
    /// Whether the mailbox sidebar is collapsed (`toggle_sidebar`).
    pub sidebar_collapsed: bool,
    /// Folders of the sidebar's tree whose children are hidden, and accounts (empty `path`)
    /// whose folders are (`collapse_folders`, `expand_folders`).
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub collapsed: BTreeSet<FolderKey>,
    /// The settings page and its open section (`open_settings`), shown in place of the list
    /// and the reader.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<SettingsSection>,
}

/// A section of the settings page.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SettingsSection {
    /// Theme, density, text size.
    #[default]
    Appearance,
    /// The key preset and the bindings.
    Keys,
    /// Undo-send window, polling, notifications, history.
    General,
    /// The accounts.
    Accounts,
}

impl SettingsSection {
    /// Every section, in tab order.
    pub const ALL: [Self; 4] = [Self::Appearance, Self::Keys, Self::General, Self::Accounts];
}

/// A pane of the main window.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Pane {
    /// Accounts and mailboxes.
    Sidebar,
    /// The thread list (a mailbox or search results).
    #[default]
    List,
    /// The open thread.
    Reader,
    /// The open draft.
    Composer,
}

impl Pane {
    /// Every pane, in focus order (`Tab`).
    pub const ALL: &'static [Self] = &[Self::Sidebar, Self::List, Self::Reader, Self::Composer];

    /// The pane's `snake_case` name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sidebar => "sidebar",
            Self::List => "list",
            Self::Reader => "reader",
            Self::Composer => "composer",
        }
    }
}

/// The part of [`ViewState`] that survives a restart: the `[view]` table of `config.toml`
/// (UX-1). View commands that change it emit `PersistState { delta: view }`; it is never part
/// of undo history.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ViewPrefs {
    /// Whether the mailbox sidebar is collapsed (`toggle_sidebar`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub sidebar_collapsed: bool,
    /// Collapsed folders and accounts (`collapse_folders`).
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub collapsed: BTreeSet<FolderKey>,
}

impl ViewPrefs {
    /// Whether every preference has its default (nothing to write).
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

impl ViewState {
    /// The preferences to save.
    pub fn prefs(&self) -> ViewPrefs {
        ViewPrefs {
            sidebar_collapsed: self.sidebar_collapsed,
            collapsed: self.collapsed.clone(),
        }
    }

    /// Restores saved preferences.
    pub fn apply_prefs(&mut self, prefs: &ViewPrefs) {
        self.sidebar_collapsed = prefs.sidebar_collapsed;
        self.collapsed = prefs.collapsed.clone();
    }
}

impl ViewState {
    /// Drops references to mailboxes and threads that no longer exist. Returns whether anything
    /// changed.
    pub fn prune(
        &mut self,
        mailbox_exists: impl Fn(&MailboxId) -> bool,
        thread_exists: impl Fn(&ThreadId) -> bool,
    ) -> bool {
        let before = self.clone();
        if self.mailbox.is_some_and(|id| !mailbox_exists(&id)) {
            self.mailbox = None;
        }
        self.selected.retain(|id| thread_exists(id));
        self.checked.retain(|id| thread_exists(id));
        if self.thread.is_some_and(|id| !thread_exists(&id)) {
            self.thread = None;
            self.expanded.clear();
        }
        *self != before
    }

    /// Whether `pane` can have the focus: the reader needs an open thread, the composer an open
    /// draft and the sidebar must not be collapsed.
    pub const fn can_focus(&self, pane: Pane) -> bool {
        match pane {
            Pane::Sidebar => !self.sidebar_collapsed,
            Pane::List => true,
            Pane::Reader => self.thread.is_some(),
            Pane::Composer => self.composer.is_some(),
        }
    }

    /// Closes a composer whose draft is gone and moves the focus off a pane that can't have it
    /// (to the composer, the reader or the list, whichever is open first). Returns whether
    /// anything changed.
    pub fn settle(&mut self, draft_exists: impl Fn(&DraftId) -> bool) -> bool {
        let before = self.clone();
        if self.composer.is_some_and(|id| !draft_exists(&id)) {
            self.composer = None;
        }
        if !self.can_focus(self.focus) {
            self.focus = [Pane::Composer, Pane::Reader, Pane::List]
                .into_iter()
                .find(|pane| self.can_focus(*pane))
                .unwrap_or_default();
        }
        *self != before
    }
}

/// The light theme a fresh profile uses.
pub const DEFAULT_LIGHT_THEME: &str = "dail-light";
/// The dark theme a fresh profile uses.
pub const DEFAULT_DARK_THEME: &str = "dail-dark";
/// The high-contrast themes the legacy `theme` values 3 and 4 name.
pub const HIGH_CONTRAST_DARK_THEME: &str = "dail-dark-hc";
/// See [`HIGH_CONTRAST_DARK_THEME`].
pub const HIGH_CONTRAST_LIGHT_THEME: &str = "dail-light-hc";

impl Settings {
    /// Maps the legacy `theme` values onto a mode and a theme name: 3 becomes dark mode with
    /// `dail-dark-hc`, 4 light mode with `dail-light-hc` (ADR 0008). Other values stay.
    pub fn migrate_legacy_theme(&mut self) {
        match self.theme {
            3 => {
                self.theme = 2;
                HIGH_CONTRAST_DARK_THEME.clone_into(&mut self.theme_dark);
            }
            4 => {
                self.theme = 1;
                HIGH_CONTRAST_LIGHT_THEME.clone_into(&mut self.theme_light);
            }
            _ => {}
        }
    }
}

/// User preferences. Missing fields take their defaults (e.g. in a hand-edited `config.toml`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Settings {
    /// Delay before a sent message actually leaves, in milliseconds (undo-send window).
    pub undo_send_ms: u64,
    /// Interval between INBOX polls, in seconds.
    pub poll_secs: u64,
    /// Number of newest messages fetched on the first sync of a mailbox.
    pub initial_window: u64,
    /// Maximum number of entries in the undo history.
    pub history_cap: u64,
    /// GUI theme mode: 0 = follow the system, 1 = light, 2 = dark. The legacy values 3
    /// (high-contrast dark) and 4 (high-contrast light) are mapped onto a mode and a theme name
    /// when read and when set (ADR 0008).
    pub theme: u64,
    /// The GUI theme for the light mode, by name (`dail-light`; `set_theme`).
    pub theme_light: String,
    /// The GUI theme for the dark mode, by name (`dail-dark`; `set_theme`).
    pub theme_dark: String,
    /// Thread list rows: 0 = cozy (two lines, 48 px), 1 = compact (one line, 40 px), 2 = preview
    /// (three lines with the snippet, 72 px).
    pub list_density: u64,
    /// TUI colours: 0 = the terminal's own (default, ADR 0004), 1 = the active theme's roles in
    /// truecolor (UI-16).
    pub tui_colors: u64,
    /// GUI text and widget size in percent.
    pub text_scale: u64,
    /// Desktop notifications while the GUI is in the background: 0 = off, 1 = on.
    pub notifications: u64,
    /// Android: mail checks while the app is not on screen: 0 = only while open, 1 = periodic
    /// (every `background_interval_mins`), 2 = push (IMAP IDLE in a foreground service). Read only
    /// by the Android host (ADR 0009).
    pub background_sync: u64,
    /// Android: minutes between periodic background checks (`background_sync` = 1).
    pub background_interval_mins: u64,
    /// Senders (lower-case addresses) whose messages show their inline images, the `cid:` parts
    /// of the message itself (`show_images_from`). Remote images are never loaded.
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub images_from: BTreeSet<String>,
    /// The key bindings the front-ends start from (`set_key_preset`).
    #[serde(skip_serializing_if = "KeyPreset::is_gmail")]
    pub key_preset: KeyPreset,
    /// Key bindings of single commands, by command name, replacing the preset's
    /// (`set_keybinding`). An empty list unbinds the command.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub keys: BTreeMap<String, Vec<String>>,
}

/// A set of key bindings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum KeyPreset {
    /// Gmail's keys: `j`/`k`, `e` archive, `#` trash, `u` read/unread, `z` undo, …
    #[default]
    Gmail,
    /// Vim-minded: as Gmail, but `u` undoes, `Ctrl+R` redoes, `d d` trashes, `m r` / `m u`
    /// mark read / unread and `F5` or `g r` syncs.
    Vim,
}

impl KeyPreset {
    /// [`KeyPreset::Gmail`].
    pub const fn is_gmail(&self) -> bool {
        matches!(self, Self::Gmail)
    }
}

impl Settings {
    /// Whether messages from `sender` show their inline images (`show_images_from`).
    pub fn shows_images_from(&self, sender: &str) -> bool {
        self.images_from.contains(&sender.trim().to_lowercase())
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            undo_send_ms: 10_000,
            poll_secs: 120,
            initial_window: 500,
            history_cap: 200,
            theme: 0,
            theme_light: DEFAULT_LIGHT_THEME.to_owned(),
            theme_dark: DEFAULT_DARK_THEME.to_owned(),
            list_density: 0,
            tui_colors: 0,
            text_scale: 100,
            notifications: 1,
            background_sync: 1,
            background_interval_mins: 30,
            images_from: BTreeSet::new(),
            key_preset: KeyPreset::Gmail,
            keys: BTreeMap::new(),
        }
    }
}

/// Runtime health.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StatusState {
    /// The last failure to persist local state, if it has not been resolved since.
    pub persist_error: Option<String>,
    /// The last failure to store or delete a password in the OS keyring, if it has not been
    /// resolved since.
    pub keyring_error: Option<String>,
    /// Sync progress and health per account.
    pub sync: IndexMap<AccountId, SyncStatus>,
    /// When the next INBOX poll is due, in Unix milliseconds (`None`: not polling; `sync_now`
    /// starts polling).
    pub next_poll_ms: Option<i64>,
    /// Messages whose body is being downloaded.
    pub bodies_in_flight: BTreeSet<MessageId>,
    /// Drafts being saved to their account's Drafts mailbox.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub drafts_saving: BTreeSet<DraftId>,
    /// Recent server searches, oldest first (at most `MAX_SERVER_SEARCHES`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub searches: Vec<ServerSearch>,
    /// Mailboxes whose older messages are being fetched (history backfill, one batch at a time).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub backfilling: BTreeSet<MailboxId>,
    /// Recent searches of the local full-text index, oldest first (at most
    /// `MAX_SERVER_SEARCHES`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub index_searches: Vec<IndexSearch>,
    /// The answer to the last `discover_servers`, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discovery: Option<Discovery>,
    /// The last `compact_store`, if any (since the session started).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compaction: Option<Compaction>,
    /// Recent `save_attachments`, oldest first, one per message (at most `MAX_SAVES`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub saves: Vec<AttachmentSave>,
}

/// A search of the local full-text index of downloaded bodies (ADR 0005).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IndexSearch {
    /// The canonical query, lower-cased.
    pub query: String,
    /// Messages whose subject, senders or body contain every word, best first.
    pub hits: Vec<MessageId>,
    /// Why the index could not be searched, if it failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// When the answer arrived, in Unix milliseconds.
    pub updated_ms: i64,
}

/// What `discover_servers` found for an address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Discovery {
    /// The address looked up.
    pub email: String,
    /// The servers, or why there are none.
    pub found: Result<DiscoveredServers, String>,
}

/// Servers `discover_servers` found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DiscoveredServers {
    /// IMAP.
    pub imap: crate::account::Endpoint,
    /// SMTP submission.
    pub smtp: crate::account::Endpoint,
    /// Where they were found.
    pub source: String,
}

/// A compaction of the local store (`compact_store`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Compaction {
    /// The runtime is compacting.
    Running {
        /// When it started, in Unix milliseconds.
        started_ms: i64,
    },
    /// It finished.
    Done {
        /// When, in Unix milliseconds.
        at_ms: i64,
        /// Bodies deleted.
        removed_blobs: u64,
        /// Bytes freed by them.
        freed_bytes: u64,
    },
    /// It failed.
    Failed {
        /// When, in Unix milliseconds.
        at_ms: i64,
        /// Diagnostic.
        error: String,
    },
}

/// Saving a message's attachments into a folder (`save_attachments`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttachmentSave {
    /// The message.
    pub message: MessageId,
    /// The folder.
    pub dir: String,
    /// Where it stands.
    pub state: SaveState,
    /// When it started or finished, in Unix milliseconds.
    pub updated_ms: i64,
}

/// The state of an [`AttachmentSave`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SaveState {
    /// The runtime is writing the files.
    Saving,
    /// Every file was written.
    Saved {
        /// The files, as absolute paths.
        files: Vec<String>,
    },
    /// Writing failed.
    Failed {
        /// The files written before the failure.
        files: Vec<String>,
        /// Diagnostic.
        error: String,
    },
}

/// A search of one account's mailboxes on the server, and the messages it found. Hits stay
/// in the cache; `search` shows them for the same query even if the match was in the body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ServerSearch {
    /// The canonical query, lower-cased.
    pub query: String,
    /// The searched account.
    pub account: AccountId,
    /// Mailbox searches still in flight.
    pub pending: u32,
    /// Cached messages the server found, in the order they were reported.
    pub hits: Vec<MessageId>,
    /// The last mailbox search that failed, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    /// When the search started or last reported, in Unix milliseconds.
    pub updated_ms: i64,
}

/// Sync progress and health of one account.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SyncStatus {
    /// Sync effects (listings, fetches) in flight. Non-zero means "syncing".
    pub pending: u32,
    /// When the last sync round finished (the moment `pending` dropped to 0), in Unix ms.
    pub last_sync_ms: Option<i64>,
    /// The last sync failure, if it has not been resolved since.
    pub last_error: Option<SyncIssue>,
    /// Push for the INBOX (IMAP IDLE): whether the server is being watched.
    #[serde(default, skip_serializing_if = "WatchState::is_off")]
    pub watch: WatchState,
}

/// Whether an account's INBOX is watched for new mail (IMAP IDLE). Polling continues for
/// accounts that aren't watched.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum WatchState {
    /// Not watched (yet): the first INBOX sync starts it.
    #[default]
    Off,
    /// The runtime waits on the server for a change.
    Watching,
    /// The last watch failed; it starts again at `at_ms`.
    Retrying {
        /// When to try again, in Unix milliseconds.
        at_ms: i64,
        /// Why the watch ended.
        error: String,
    },
    /// The server has no IDLE: polling only.
    Unsupported,
}

impl WatchState {
    /// [`WatchState::Off`].
    pub const fn is_off(&self) -> bool {
        matches!(self, Self::Off)
    }
}

impl SyncStatus {
    /// Whether sync effects are in flight.
    pub const fn is_syncing(&self) -> bool {
        self.pending > 0
    }
}

/// A failed sync step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SyncIssue {
    /// Diagnostic from the server or the connection.
    pub error: String,
    /// The mailbox involved, if the step concerned one.
    pub mailbox: Option<MailboxId>,
    /// When the failure was reported, in Unix milliseconds.
    pub at_ms: i64,
}
