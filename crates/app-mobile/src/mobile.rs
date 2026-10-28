//! `Mobile`: the presenter's UI-local state, and the projection of `AppState` into a [`Frame`].
//!
//! Navigation is the `ViewState`, as in the TUI: the drawer is `focus_pane sidebar`, a thread is
//! `open_thread`, and Back is the `Escape` binding. The window class only chooses panes
//! (`app_front::panes`); it never dispatches a command.

use std::collections::HashSet;

use app_core::document::FoldKind;
use app_core::{
    AccountId, Addr, AppState, AttachmentState, Command, CommandName, Draft, DraftId, FolderKey,
    MailboxRole, MessageId, Pane, QuoteStyle, SettingsSection, ThreadId, ThreadSummary, listing,
};
use app_front::body::Bodies;
use app_front::i18n::Lang;
use app_front::keymap::{Action, Chord, Key, KeyState, Press, parse_chord};
use app_front::mailbox::{self, mailbox_name};
use app_front::notice::{self, LastAction};
use app_front::panes::{self, Slot, Width};
use app_front::status::{self, Fix};
use app_front::text::{clean, clean_lines};
use app_front::{a11y, compose, format, palette, updates};

use crate::composer::Buffers;
use crate::event::{External, Field, UiEvent};
use crate::frame::{
    ActionId, Banner, Bulk, Button, Chip, Choice, Class, Composer, Density, Drawer, DrawerAccount,
    DrawerItem, Fold, Frame, Layout, List, OutgoingFile, PaneKind, QuoteFold, Recipients, Row,
    Snackbar, TopBar,
};
use crate::request::{Purpose, Reply, Request};

mod reader;
pub use reader::preview_text;
mod settings;
mod setup;
mod sheet;

/// How long a notice or an undo offer stays in the snackbar.
const NOTICE_MS: i64 = 6_000;

/// Rows sent around the visible ones, before and after.
const MARGIN: usize = 20;

/// Rows the list window holds before the host reports what is visible.
const FIRST_WINDOW: usize = 30;

/// What an action ID does.
#[derive(Debug, Clone, PartialEq)]
enum Act {
    /// Submit this command.
    Run(Command),
    /// Submit these commands in order (open a mailbox, then close the modal drawer).
    Many(Vec<Command>),
    /// What Back does.
    Back,
    /// Start a new message from the current account (`create_draft`, then the composer).
    Compose,
    /// Open the About screen.
    About,
    /// Maximize the message pane, or restore the panes (MISC-14).
    Maximize,
    /// Add a suggested contact to a recipient field.
    Suggest(Field, Addr),
    /// Expand or fold the composer's quote.
    ToggleQuote,
    /// Ask the host for files to attach to a draft.
    Pick(DraftId),
    /// Choose a row of the palette.
    Choose(palette::Target),
    /// Leave the palette's step, else close it.
    PaletteBack,
    /// Open the palette at this command's step (*Move…*).
    Flow(CommandName),
    /// Open or fold a message's quote or signature.
    Fold(MessageId, FoldKind),
    /// Show a link's full target (the link sheet).
    Link(String),
    /// Open the link of the sheet.
    OpenLink(String),
    /// Copy the link of the sheet.
    CopyLink(String),
    /// Show a message's attachment in the app.
    Preview(MessageId, usize),
    /// Save attachments of the body with this blob.
    Save(Option<String>, Vec<crate::event::SaveFile>),
    /// A button of the account setup.
    Setup(setup::SetupAct),
    /// Open the password form of an account.
    Reenter(AccountId),
    /// Ask to confirm removing an account (the palette's confirmation step).
    RemoveAccount(AccountId),
    /// Open a screen of the system's settings.
    System(crate::event::SystemScreen),
    /// Switch a query's result between its view and its raw JSON (MISC-22).
    RawResult,
    /// A row of a query's result: close the result and run its command.
    FromResult(Command),
}

/// The window, as the host last configured it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Window {
    width_dp: u32,
    height_dp: u32,
}

/// The thread list's rows, kept until the state's revision changes (`listing` is O(n)).
#[derive(Debug, Default)]
struct RowCache {
    revision: Option<u64>,
    ids: Vec<ThreadId>,
}

impl RowCache {
    fn get(&mut self, state: &AppState) -> &[ThreadId] {
        let revision = state.revision.get();
        if self.revision != Some(revision) {
            self.ids = listing::visible_threads(state);
            self.revision = Some(revision);
        }
        &self.ids
    }
}

/// UI-local state that is not part of `AppState`.
pub struct Mobile {
    clock: fn() -> i64,
    lang: Lang,
    window: Window,
    visible: bool,
    /// The first visible row and how many are visible (`scrolled`).
    scrolled: (usize, usize),
    rows: RowCache,
    /// The action table of the last frame.
    actions: Vec<(ActionId, Act)>,
    next_action: ActionId,
    revision: u64,
    bodies: Bodies,
    keys: KeyState,
    last: LastAction,
    /// A message for the snackbar, until the given time (Unix ms).
    notice: Option<(String, i64)>,
    /// Until when the snackbar offers to undo the latest action.
    undo_until: i64,
    /// The draft whose send this presenter scheduled, while it can be cancelled.
    sending: Option<DraftId>,
    externals: Vec<External>,
    /// The system's dark mode (`configure`).
    dark: bool,
    /// The theme the host has, by slug.
    theme_sent: Option<String>,
    /// Whether the About screen is open.
    about: bool,
    /// Panes a pinch took away from the reader (MISC-6): 0 none, 1 the sidebar, 2 the list too.
    /// Kept across threads.
    pane_step: u8,
    /// The composer's typed text.
    compose: Buffers,
    /// The drafts whose scheduled or leaving send the host was told about (`sending`).
    kept: Vec<DraftId>,
    /// The background schedule last told to the host (`External::Schedule`).
    schedule: Option<(u64, u64)>,
    /// The send errors already told in the snackbar, by draft.
    told: Vec<(DraftId, String)>,
    /// The palette's query and step.
    palette: palette::Palette,
    /// The version of the palette's query field; changes when the presenter replaces it.
    palette_version: u64,
    /// Whether the palette was open at the last look (to forget its step once closed).
    palette_open: bool,
    /// A query's result, until Back.
    result: Option<sheet::Result>,
    /// The folds (quotes, signatures) the user opened, per message.
    folds: HashSet<(MessageId, FoldKind)>,
    /// The tapped link, while its sheet is open.
    link: Option<String>,
    /// The attachment shown in the app, by message and part.
    preview: Option<(MessageId, usize)>,
    /// The account setup, while open.
    setup: Option<setup::Setup>,
    /// Whether the setup was opened on its own for a profile without accounts (once).
    offered: bool,
}

impl Default for Mobile {
    fn default() -> Self {
        Self::with_clock(system_now_ms)
    }
}

impl std::fmt::Debug for Mobile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mobile")
            .field("revision", &self.revision)
            .finish_non_exhaustive()
    }
}

fn system_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// Commands whose receipt carries a new draft to show in the composer.
const fn creates_draft(name: CommandName) -> bool {
    matches!(
        name,
        CommandName::CreateDraft
            | CommandName::Reply
            | CommandName::ReplyAll
            | CommandName::Forward
    )
}

/// The icon of a mailbox in the drawer.
const fn mailbox_icon(role: Option<MailboxRole>) -> &'static str {
    match role {
        Some(MailboxRole::Inbox) => "inbox",
        Some(MailboxRole::Sent) => "send",
        Some(MailboxRole::Drafts) => "draft",
        Some(MailboxRole::Trash) => "delete",
        Some(MailboxRole::Archive) => "archive",
        Some(MailboxRole::Junk) => "report",
        Some(MailboxRole::All) => "all_inbox",
        Some(MailboxRole::Flagged) => "star",
        None => "folder",
    }
}

impl Mobile {
    /// A presenter that reads the time from `clock` (Unix ms) instead of the system clock.
    pub fn with_clock(clock: fn() -> i64) -> Self {
        Self {
            clock,
            lang: Lang::default(),
            window: Window {
                width_dp: 360,
                height_dp: 800,
            },
            visible: true,
            scrolled: (0, FIRST_WINDOW),
            rows: RowCache::default(),
            actions: Vec::new(),
            next_action: 1,
            revision: 0,
            bodies: Bodies::rich(),
            keys: KeyState::default(),
            last: LastAction::default(),
            notice: None,
            undo_until: 0,
            sending: None,
            externals: Vec::new(),
            dark: false,
            theme_sent: None,
            about: false,
            pane_step: 0,
            compose: Buffers::default(),
            told: Vec::new(),
            kept: Vec::new(),
            schedule: None,
            palette: palette::Palette::default(),
            palette_version: 1,
            palette_open: false,
            result: None,
            folds: HashSet::new(),
            link: None,
            preview: None,
            setup: None,
            offered: false,
        }
    }

    /// The time the presenter shows (Unix ms).
    pub fn now_ms(&self) -> i64 {
        (self.clock)()
    }

    /// The window class of the last `configure`.
    pub const fn class(&self) -> Class {
        Class::of(self.window.width_dp)
    }

    /// Whether the app is on screen (the last `visible`).
    pub const fn visible(&self) -> bool {
        self.visible
    }

    /// The requests the state calls for without an event (bodies for the open thread). The
    /// host calls this before each frame.
    pub fn refresh(&mut self, state: &AppState) -> Vec<Request> {
        self.bodies
            .refresh(state)
            .map(|(thread, cmd)| Request {
                purpose: Purpose::Bodies(thread),
                cmd,
            })
            .into_iter()
            .collect()
    }

    /// Shows `text` in the snackbar for a while: something the host did on its own failed
    /// (a picked file that could not be copied).
    pub fn notify(&mut self, text: &str) {
        self.notice = Some((clean(text), self.now_ms() + NOTICE_MS));
    }

    /// What only the host can do, asked for since the last call.
    pub fn externals(&mut self) -> Vec<External> {
        std::mem::take(&mut self.externals)
    }

    /// Turns one event into what to submit.
    pub fn event(&mut self, event: UiEvent, state: &AppState) -> Vec<Request> {
        self.compose.sync(state);
        self.palette_sync(state);
        let acts = matches!(
            event,
            UiEvent::Tap { .. }
                | UiEvent::LongPress { .. }
                | UiEvent::Swipe { .. }
                | UiEvent::Back
                | UiEvent::Refresh
                | UiEvent::Notification { .. }
                | UiEvent::Key { .. }
                | UiEvent::KeyDown { .. }
        );
        // Typed text goes back before any command, so the command sees it.
        let flushed: Vec<Request> = if acts {
            self.compose.flush(state).into_iter().collect()
        } else {
            Vec::new()
        };
        let mut out = self.handle(event, state);
        if !flushed.is_empty() {
            out.splice(0..0, flushed);
        }
        out
    }

    fn handle(&mut self, event: UiEvent, state: &AppState) -> Vec<Request> {
        let now = self.now_ms();
        match event {
            UiEvent::Configure {
                width_dp,
                height_dp,
                dark,
                locale,
                ..
            } => {
                self.dark = dark;
                self.window = Window {
                    width_dp,
                    height_dp,
                };
                self.lang = Lang::from_tag(locale.as_deref());
                Vec::new()
            }
            UiEvent::Visible { visible } => {
                self.visible = visible;
                Vec::new()
            }
            UiEvent::Tap { action } => self.act(action, state),
            UiEvent::LongPress { action } | UiEvent::Swipe { action } => {
                let out = self.act(action, state);
                if !out.is_empty() {
                    self.externals.push(External::Haptic);
                }
                out
            }
            UiEvent::Pinch { zoom_in } => {
                self.pane_step = if zoom_in {
                    (self.pane_step + 1).min(2)
                } else {
                    self.pane_step.saturating_sub(1)
                };
                Vec::new()
            }
            UiEvent::Scrolled { first, count } => {
                self.scrolled = (first, count);
                Vec::new()
            }
            UiEvent::Back => self.back(state),
            UiEvent::Refresh => vec![Request::user(Command::SyncNow { account: None })],
            UiEvent::Notification { act } => {
                act.commands(state).into_iter().map(Request::user).collect()
            }
            UiEvent::Online => {
                if self.visible && !state.accounts.is_empty() {
                    vec![Request::user(Command::SyncNow { account: None })]
                } else {
                    Vec::new()
                }
            }
            UiEvent::Key { chord } => match parse_chord(&chord) {
                Some(chord) => self.press(chord, state),
                None => Vec::new(),
            },
            UiEvent::KeyDown {
                code,
                unicode,
                ctrl,
                shift,
                alt,
                meta,
            } => {
                let key = crate::keys::AndroidKey {
                    code,
                    unicode,
                    ctrl,
                    shift,
                    alt,
                    meta,
                };
                match crate::keys::chord_of(key) {
                    Some(chord) => self.press(chord, state),
                    None => Vec::new(),
                }
            }
            UiEvent::Text {
                field: Field::Query,
                text,
            } => {
                self.palette_query(text);
                Vec::new()
            }
            UiEvent::Text {
                field:
                    field @ (Field::Email | Field::Name | Field::Imap | Field::Smtp | Field::Password),
                text,
            } => {
                if let Some(setup) = self.setup.as_mut() {
                    setup.text(field, text);
                }
                Vec::new()
            }
            UiEvent::Blur {
                field: Field::Email | Field::Name | Field::Imap | Field::Smtp | Field::Password,
            } => Vec::new(),
            UiEvent::Text { field, text } => self.compose.text(field, text, state, now),
            UiEvent::Blur { field } => self.compose.blur(field, state),
            UiEvent::Tick => self.compose.idle(state, now).into_iter().collect(),
            UiEvent::Share {
                mailto,
                to,
                subject,
                text,
                files,
            } => self.share(state, mailto.as_deref(), &to, subject, text, files),
        }
    }

    /// Takes the results of earlier requests; returns what they lead to (a new draft opens in
    /// the composer, as in the GUI).
    pub fn replies(&mut self, replies: Vec<Reply>, _state: &AppState) -> Vec<Request> {
        let mut out = Vec::new();
        let now = self.now_ms();
        for mut reply in replies {
            if matches!(reply.purpose, Purpose::Shared(_))
                && let Purpose::Shared(files) = std::mem::replace(&mut reply.purpose, Purpose::User)
            {
                let draft = reply
                    .result
                    .as_ref()
                    .ok()
                    .and_then(|r| r.as_ref().ok())
                    .and_then(|r| serde_json::from_value(r.data["id"].clone()).ok());
                if let Some(draft) = draft
                    && !files.is_empty()
                {
                    self.externals
                        .push(External::ImportFiles { draft, uris: files });
                }
            }
            match reply.purpose {
                Purpose::Setup => {
                    if let Some(next) = self.setup.as_mut().and_then(|s| s.accept(&reply)) {
                        out.push(next);
                    }
                }
                Purpose::Bodies(thread) => {
                    let receipt = reply.result.as_ref().ok().and_then(|r| r.as_ref().ok());
                    self.bodies.accept(thread, receipt);
                }
                // A share's draft is a user action once its files are handed on.
                Purpose::User | Purpose::Shared(_) => match &reply.result {
                    Ok(Ok(receipt)) => {
                        let before = self.last.label().map(str::to_owned);
                        if let Some(draft) = self.last.observe(receipt) {
                            self.sending = Some(draft);
                        }

                        if self.last.label().is_some() && self.last.label() != before.as_deref() {
                            self.undo_until = now + NOTICE_MS;
                        }
                        if let Some(result) = sheet::Result::of(reply.name, receipt) {
                            self.result = Some(result);
                        }
                        if creates_draft(reply.name)
                            && let Ok(id) = serde_json::from_value(receipt.data["id"].clone())
                        {
                            out.push(Request::user(Command::OpenComposer { id }));
                        }
                    }
                    Ok(Err(err)) => {
                        let text = notice::rejection(reply.name.spec().title, err);
                        self.notice = Some((text, now + NOTICE_MS));
                    }
                    Err(err) => self.notice = Some((format!("dail: {err}"), now + NOTICE_MS)),
                },
            }
        }
        out
    }

    /// Whether the screen changes with the clock: a send counts down, an age is shown, or a
    /// message is about to leave the snackbar.
    pub fn wants_tick(&self, state: &AppState) -> bool {
        let now = self.now_ms();
        !state.accounts.is_empty()
            || state.drafts.values().any(|d| d.send_at.is_some())
            || self.notice.as_ref().is_some_and(|(_, until)| *until > now)
            || self.undo_until > now
            || self.compose.is_dirty()
    }

    fn act(&mut self, action: ActionId, state: &AppState) -> Vec<Request> {
        let Some(act) = self
            .actions
            .iter()
            .find(|(id, _)| *id == action)
            .map(|(_, act)| act.clone())
        else {
            return Vec::new();
        };
        // About is over everything: a command from elsewhere (a mailbox in the drawer) moves on
        // from it, or the panes would change unseen beneath it (MISC-12).
        if matches!(act, Act::Run(_) | Act::Many(_)) {
            self.about = false;
        }
        match act {
            Act::Run(cmd) => vec![Request::user(cmd)],
            Act::RawResult => {
                if let Some(result) = &mut self.result {
                    result.raw = !result.raw;
                }
                Vec::new()
            }
            Act::FromResult(cmd) => {
                self.result = None;
                vec![Request::user(cmd)]
            }
            Act::Many(cmds) => cmds.into_iter().map(Request::user).collect(),
            Act::Back => self.back(state),
            Act::Maximize => {
                // The largest pinch step (MISC-6), or back to none; it stays across threads.
                self.pane_step = if self.pane_step >= 2 { 0 } else { 2 };
                Vec::new()
            }
            Act::About => {
                self.about = true;
                Vec::new()
            }
            Act::Suggest(field, addr) => self.compose.suggest(field, addr, state),
            Act::ToggleQuote => {
                self.compose.quote_open = !self.compose.quote_open;
                Vec::new()
            }
            Act::Choose(target) => self.palette_choose(target, state),
            Act::PaletteBack => self.palette_back(),
            Act::Flow(name) => self.palette_flow(name, state),
            Act::Fold(message, kind) => {
                self.toggle_fold(message, kind);
                Vec::new()
            }
            Act::Link(url) => {
                self.link = Some(url);
                Vec::new()
            }
            Act::OpenLink(url) => self.open_link(&url, state),
            Act::CopyLink(url) => {
                self.copy_link(url);
                Vec::new()
            }
            Act::Preview(message, part) => {
                self.preview = Some((message, part));
                Vec::new()
            }
            Act::Save(blob, files) => {
                self.save(blob, files);
                Vec::new()
            }
            Act::Setup(act) => self.setup_act(act),
            Act::Reenter(account) => {
                self.open_reenter(state, account);
                Vec::new()
            }
            Act::RemoveAccount(account) => self.ask(palette::remove_account(state, account), state),
            Act::System(screen) => {
                self.system(screen);
                Vec::new()
            }
            Act::Pick(draft) => {
                self.externals.push(External::PickFiles { draft });
                Vec::new()
            }
            Act::Compose => palette::current_account(state)
                .map(|account| {
                    Request::user(Command::CreateDraft {
                        account,
                        to: Vec::new(),
                        cc: Vec::new(),
                        bcc: Vec::new(),
                        subject: String::new(),
                        body: String::new(),
                    })
                })
                .into_iter()
                .collect(),
        }
    }

    /// A new draft from a share or a `mailto:` link; the files follow once it exists.
    fn share(
        &mut self,
        state: &AppState,
        mailto: Option<&str>,
        to: &[String],
        subject: Option<String>,
        text: Option<String>,
        files: Vec<String>,
    ) -> Vec<Request> {
        let Some(account) = palette::current_account(state) else {
            self.notify("Add an account in dail before sharing to it.");
            return Vec::new();
        };
        let mut fields = mailto
            .and_then(app_front::mailto::parse)
            .unwrap_or_default();
        for addr in to.iter().filter_map(|a| compose::parse_addr(a)) {
            if !fields
                .to
                .iter()
                .any(|a| a.email.eq_ignore_ascii_case(&addr.email))
            {
                fields.to.push(addr);
            }
        }
        if let Some(subject) = subject.filter(|s| !s.trim().is_empty()) {
            fields.subject = subject;
        }
        if let Some(text) = text.filter(|t| !t.trim().is_empty()) {
            fields.body = if fields.body.is_empty() {
                text
            } else {
                format!("{}\n\n{text}", fields.body)
            };
        }
        let total = files.len();
        let files: Vec<String> = files.into_iter().filter(|u| shareable(u)).collect();
        if files.len() < total {
            let refused = total - files.len();
            self.notify(&format!(
                "{refused} shared file{} could not be attached: only files shared by other apps can.",
                if refused == 1 { "" } else { "s" }
            ));
        }
        vec![Request {
            purpose: Purpose::Shared(files),
            cmd: Command::CreateDraft {
                account,
                to: fields.to,
                cc: fields.cc,
                bcc: fields.bcc,
                subject: fields.subject,
                body: fields.body,
            },
        }]
    }

    fn press(&mut self, chord: Chord, state: &AppState) -> Vec<Request> {
        let now = self.now_ms();
        let typing = state.view.composer.is_some() && state.view.focus == Pane::Composer;
        match self.keys.press(chord, typing, state, now) {
            Press::Done(Action::Run(cmd)) => vec![Request::user(cmd)],
            Press::Done(Action::Ask(step)) => self.ask(palette::Flow::Ask(step), state),
            Press::Done(Action::Wizard) => self.ask(palette::Flow::Wizard, state),
            Press::Done(Action::Password(id)) => self.ask(palette::Flow::Password(id), state),
            Press::Pending | Press::Unbound => Vec::new(),
        }
    }

    /// What Back submits: the `Escape` binding (close the composer, the thread, the search, …),
    /// else uncheck the checked threads, else from the drawer of a one-pane window back to the list.
    fn back(&mut self, state: &AppState) -> Vec<Request> {
        if self.about {
            self.about = false;
            return Vec::new();
        }
        if let Some(setup) = self.setup.as_mut() {
            if !setup.back() {
                self.setup = None;
            }
            return Vec::new();
        }
        if self.result.take().is_some()
            || self.link.take().is_some()
            || self.preview.take().is_some()
        {
            return Vec::new();
        }
        if state.view.palette_open {
            return self.palette_back();
        }
        // A modal drawer closes before what it covers.
        if self.drawer_back(state) {
            return vec![Request::user(Command::FocusPane { pane: Pane::List })];
        }
        if let Some(cmd) = self.escape(state) {
            return vec![Request::user(cmd)];
        }
        if !state.view.checked.is_empty() {
            return vec![Request::user(Command::UncheckThreads { ids: Vec::new() })];
        }
        Vec::new()
    }

    fn escape(&mut self, state: &AppState) -> Option<Command> {
        self.keys.sync(&state.settings);
        let escape = Chord::plain(Key::Escape);
        self.keys
            .bindings()
            .iter()
            .filter(|b| b.keys == [escape])
            .find_map(|b| match app_front::keymap::resolve(b, state) {
                Some(Action::Run(cmd)) => Some(cmd),
                _ => None,
            })
    }

    /// Whether the drawer is modal and open: compact and medium windows, the sidebar focused.
    fn drawer_back(&self, state: &AppState) -> bool {
        self.class() != Class::Expanded
            && state.view.focus == Pane::Sidebar
            && !state.accounts.is_empty()
    }

    fn add(&mut self, act: Act) -> ActionId {
        let id = self.next_action;
        self.next_action = self.next_action.wrapping_add(1).max(1);
        self.actions.push((id, act));
        id
    }

    fn button(&mut self, icon: &str, label: &str, act: Act) -> Button {
        Button {
            icon: Some(icon.to_owned()),
            label: label.to_owned(),
            action: self.add(act),
        }
    }

    /// The next screen. The action IDs of earlier frames stop working.
    pub fn frame(&mut self, state: &AppState) -> Frame {
        self.compose.sync(state);
        self.palette_sync(state);
        self.actions.clear();
        self.revision += 1;
        if !self.offered && state.accounts.is_empty() {
            self.offered = true;
            self.open_setup();
        }
        let class = self.class();
        let width = match class {
            Class::Compact => Width::Narrow,
            Class::Medium => Width::Medium,
            Class::Expanded => Width::Wide,
        };
        let modal = self.drawer_back(state);
        let slots = if state.accounts.is_empty() {
            vec![Slot::List]
        } else if modal {
            panes::behind_drawer(width, state)
        } else {
            panes::choose(width, state)
        };
        let mut kinds: Vec<PaneKind> = slots
            .iter()
            .map(|slot| match slot {
                Slot::Mailboxes => PaneKind::Drawer,
                Slot::List => PaneKind::List,
                Slot::Reader => PaneKind::Reader,
                Slot::Composer => PaneKind::Composer,
            })
            .collect();
        // The settings page takes the place of the list and the detail, as in the GUI.
        if state.view.settings.is_some() && !state.accounts.is_empty() {
            kinds.retain(|k| *k == PaneKind::Drawer);
            kinds.push(PaneKind::Settings);
        }
        // A landscape window that is not compact always has the left pane (MISC-6). There, and
        // on any expanded window, `toggle_sidebar` switches it between the full tree and an
        // icon rail (`sidebar_collapsed`, persisted in `[view]`; MISC-11).
        let landscape = self.window.width_dp > self.window.height_dp;
        let side = !state.accounts.is_empty()
            && !modal
            && (class == Class::Expanded || (landscape && class != Class::Compact));
        let rail = side && state.view.sidebar_collapsed;
        if (rail || (side && landscape)) && !kinds.contains(&PaneKind::Drawer) {
            kinds.insert(0, PaneKind::Drawer);
        }
        // Whether there is anything beside the message to hide (MISC-14).
        let crowded = kinds.contains(&PaneKind::Reader) && kinds.len() > 1;
        // A pinch gave the message more room (MISC-6).
        if kinds.contains(&PaneKind::Reader) {
            if self.pane_step >= 1 {
                kinds.retain(|k| *k != PaneKind::Drawer);
            }
            if self.pane_step >= 2 && kinds.len() > 1 {
                kinds.retain(|k| *k != PaneKind::List);
            }
        }
        // On a phone the open message is a sheet over the list, which keeps its place (MISC-17).
        let sheet = (class == Class::Compact && kinds == [PaneKind::Reader]).then(|| {
            kinds = vec![PaneKind::List];
            PaneKind::Reader
        });
        let maximized = self.pane_step >= 2 && kinds == [PaneKind::Reader];
        let mut top_bar = self.top_bar(state, &kinds);
        if maximized {
            // Minimal: the message has the room; its own bar keeps its actions.
            top_bar.actions.clear();
        }
        let chosen = crate::theme::chosen(&state.settings, self.dark);
        // A modal drawer gives way to the About screen it opened.
        let drawer = ((modal && !self.about) || kinds.contains(&PaneKind::Drawer))
            .then(|| self.drawer(state, modal, rail, side, chosen.tags.len()));
        let list = kinds
            .contains(&PaneKind::List)
            .then(|| self.list(state, chosen.tags.len()));
        let mut reader = if kinds.contains(&PaneKind::Reader) || sheet.is_some() {
            self.reader(state)
        } else {
            None
        };
        if let Some(reader) = reader.as_mut()
            && (crowded || maximized)
        {
            reader.maximize = Some(if maximized {
                self.button("close_fullscreen", "Restore panes", Act::Maximize)
            } else {
                self.button("open_in_full", "Maximize message", Act::Maximize)
            });
        }
        let composer = if kinds.contains(&PaneKind::Composer) {
            self.composer(state)
        } else {
            None
        };
        let settings = if kinds.contains(&PaneKind::Settings) {
            self.settings_page(state)
        } else {
            None
        };
        let fab = (!self.about
            && sheet.is_none()
            && self.setup.is_none()
            && !state.accounts.is_empty()
            && !kinds.contains(&PaneKind::Composer)
            && !kinds.contains(&PaneKind::Settings))
        .then(|| self.button("edit", "Compose", Act::Compose));
        self.tell_send_errors(state);
        self.keep_sends(state);
        self.tell_schedule(state);
        let bulk = self.bulk(state);
        let banner = self.banner(state);
        let snackbar = self.snackbar(state);
        let theme = (self.theme_sent.as_deref() != Some(&*chosen.id)).then(|| {
            self.theme_sent = Some(chosen.id.to_string());
            crate::theme::frame_theme(&chosen)
        });
        let about = self.about.then(crate::theme::about);
        let setup = self.setup_sheet(state);
        let palette = self.palette_sheet(state);
        let result = self.result_view(state);
        if reader.is_none() {
            self.link = None;
            self.preview = None;
        }
        let link = self.link_sheet();
        let preview = self.preview_view(state);
        if preview.is_none() {
            self.preview = None;
        }
        let back = self.about
            || setup.is_some()
            || palette.is_some()
            || result.is_some()
            || link.is_some()
            || preview.is_some()
            || self.escape(state).is_some()
            || !state.view.checked.is_empty()
            || self.drawer_back(state);
        Frame {
            revision: self.revision,
            layout: Layout {
                class,
                panes: kinds,
                sheet,
            },
            top_bar,
            drawer,
            list,
            reader,
            composer,
            settings,
            setup,
            bulk,
            fab,
            banner,
            snackbar,
            theme,
            about,
            palette,
            result,
            link,
            preview,
            back,
            wants_tick: self.wants_tick(state),
        }
    }

    fn top_bar(&mut self, state: &AppState, kinds: &[PaneKind]) -> TopBar {
        if self.about {
            // MISC-3: the About screen closes from its own top bar, not only with Back.
            return TopBar {
                // `× About dail (<version>)` (MISC-13).
                title: crate::theme::about_title(),
                subtitle: None,
                navigation: Some(self.button("close", "Close", Act::Back)),
                actions: Vec::new(),
            };
        }
        let one = match kinds {
            [only] => Some(*only),
            _ => None,
        };
        let title = match one {
            Some(PaneKind::Drawer) => "Mailboxes".to_owned(),
            Some(PaneKind::Reader) => "Message".to_owned(),
            Some(PaneKind::Composer) => "Draft".to_owned(),
            Some(PaneKind::Settings) => "Settings".to_owned(),
            Some(PaneKind::List) | None => list_title(state),
        };
        let subtitle = (!state.accounts.is_empty())
            .then(|| status::sync_text(state, self.now_ms(), self.lang))
            .filter(|s| !s.is_empty());
        let navigation = match one {
            _ if state.accounts.is_empty() => None,
            Some(PaneKind::Reader | PaneKind::Composer | PaneKind::Drawer | PaneKind::Settings) => {
                Some(self.button("arrow_back", "Back", Act::Back))
            }
            Some(PaneKind::List) | None if !kinds.contains(&PaneKind::Drawer) => Some(self.button(
                "menu",
                "Mailboxes",
                Act::Run(Command::FocusPane {
                    pane: Pane::Sidebar,
                }),
            )),
            Some(PaneKind::List) | None => None,
        };
        let mut actions = Vec::new();
        if !state.accounts.is_empty() {
            actions.push(self.button(
                "search",
                "Search or run a command",
                Act::Run(Command::OpenPalette {}),
            ));
        }
        if state.accounts.is_empty() {
            actions.push(Button {
                icon: None,
                label: "Add account".to_owned(),
                action: self.add(Act::Setup(setup::SetupAct::Open)),
            });
        }
        if state.accounts.is_empty() {
            // Without accounts there is no drawer: *About* stays here (MISC-3).
            actions.push(Button {
                icon: None,
                label: "About".to_owned(),
                action: self.add(Act::About),
            });
        }
        TopBar {
            title,
            subtitle,
            navigation,
            actions,
        }
    }

    /// The chevron of a folder or an account: `collapse_folders` or `expand_folders` for `key`.
    /// It never closes a modal drawer.
    fn fold(&mut self, state: &AppState, key: FolderKey, a11y: String) -> Fold {
        let collapsed = state.view.collapsed.contains(&key);
        let keys = vec![key];
        let cmd = if collapsed {
            Command::ExpandFolders { keys }
        } else {
            Command::CollapseFolders { keys }
        };
        Fold {
            collapsed,
            action: self.add(Act::Run(cmd)),
            a11y,
        }
    }

    /// The drawer. A tap in a modal drawer also closes it (`focus_pane list`).
    fn drawer(
        &mut self,
        state: &AppState,
        modal: bool,
        rail: bool,
        side: bool,
        tags: usize,
    ) -> Drawer {
        // A mailbox opened while the settings page is up leaves it, or the page would stay in
        // place of the list with no way back but Back (iOS has none).
        let leave_settings = state.view.settings.is_some();
        let open = |cmd: Command| {
            let mut cmds = Vec::new();
            if leave_settings && !matches!(cmd, Command::OpenSettings { .. }) {
                cmds.push(Command::CloseSettings {});
            }
            cmds.push(cmd);
            if modal {
                cmds.push(Command::FocusPane { pane: Pane::List });
            }
            match <[Command; 1]>::try_from(cmds) {
                Ok([cmd]) => Act::Run(cmd),
                Err(cmds) => Act::Many(cmds),
            }
        };
        let browsing = state.view.search.is_none() && state.view.settings.is_none();
        let several = state.accounts.len() > 1;
        let all_inboxes = several.then(|| {
            let unread = state
                .mailboxes
                .values()
                .filter(|m| m.role == Some(MailboxRole::Inbox))
                .map(|m| m.unread)
                .sum();
            let label = "All inboxes".to_owned();
            DrawerItem {
                a11y: a11y::nav_row(&label, unread),
                label,
                depth: 0,
                fold: None,
                icon: "all_inbox".to_owned(),
                unread,
                selected: state.view.unified && browsing,
                action: self.add(open(Command::OpenUnifiedInbox {})),
            }
        });
        let mut accounts = Vec::new();
        for (index, account) in state.accounts.values().enumerate() {
            if !mailbox::shows_account(state, account.id) {
                continue;
            }
            let name = clean(&account.display_name);
            // The folder tree, as the desktop sidebar shows it (MISC-10).
            let nodes = mailbox::sidebar_tree(state, account.id);
            let mut mailboxes = Vec::new();
            for node in mailbox::visible_rows(state, account.id, &nodes) {
                let label = clean(&node.label);
                let fold = node
                    .is_parent()
                    .then(|| self.fold(state, node.key.clone(), format!("{label} subfolders")));
                let action = match (node.mailbox, &fold) {
                    (Some(id), _) => self.add(open(Command::OpenMailbox { id })),
                    (None, Some(fold)) => fold.action,
                    (None, None) => continue,
                };
                mailboxes.push(DrawerItem {
                    a11y: a11y::nav_row(&label, node.unread),
                    label,
                    depth: node.depth,
                    fold,
                    icon: mailbox_icon(node.role).to_owned(),
                    unread: node.unread,
                    selected: !state.view.unified
                        && node.mailbox.is_some()
                        && state.view.mailbox == node.mailbox
                        && browsing,
                    action,
                });
            }
            let key = FolderKey {
                account: account.id,
                path: String::new(),
            };
            let fold =
                (!nodes.is_empty()).then(|| self.fold(state, key, format!("{name} folders")));
            accounts.push(DrawerAccount {
                name,
                email: clean(&account.email),
                tag: (several && tags > 0).then(|| index % tags),
                fold,
                mailboxes,
            });
        }
        let mut footer = Vec::new();
        for (label, icon, section) in [
            ("Settings", "settings", None),
            ("Add account", "add", Some(SettingsSection::Accounts)),
        ] {
            let selected = section.is_none() && state.view.settings.is_some() && !self.about;
            footer.push(DrawerItem {
                a11y: label.to_owned(),
                label: label.to_owned(),
                depth: 0,
                fold: None,
                icon: icon.to_owned(),
                unread: 0,
                selected,
                action: self.add(open(Command::OpenSettings { section })),
            });
            if section.is_none() {
                // MISC-3: *About* right after *Settings* (no command: the screen is the
                // presenter's).
                footer.push(DrawerItem {
                    a11y: "About".to_owned(),
                    label: "About".to_owned(),
                    depth: 0,
                    fold: None,
                    icon: "info".to_owned(),
                    unread: 0,
                    selected: self.about,
                    action: self.add(Act::About),
                });
            }
        }
        // The rail ↔ full tree switch, where the pane can be either (MISC-11).
        let style = side.then(|| {
            let (icon, label) = if rail {
                ("chevron_right", "Show folder names")
            } else {
                ("chevron_left", "Icons only")
            };
            self.button(icon, label, Act::Run(Command::ToggleSidebar {}))
        });
        Drawer {
            modal,
            rail,
            style,
            all_inboxes,
            accounts,
            footer,
        }
    }

    fn list(&mut self, state: &AppState, tags: usize) -> List {
        let title = list_title(state);
        let density = match state.settings.list_density {
            1 => Density::Compact,
            2 => Density::Preview,
            _ => Density::Cozy,
        };
        if state.accounts.is_empty() {
            return List {
                title,
                total: 0,
                offset: 0,
                density,
                checked: 0,
                refreshing: false,
                rows: Vec::new(),
                server: None,
                empty: Some("No accounts yet. Add one to read your mail here.".to_owned()),
            };
        }
        let ids = self.rows.get(state).to_vec();
        let total = ids.len();
        let (first, count) = self.scrolled;
        let offset = first.saturating_sub(MARGIN).min(total);
        let end = first
            .saturating_add(count)
            .saturating_add(MARGIN)
            .min(total);
        let now = self.now_ms();
        let cursor = state.view.selected.last().copied().or(state.view.thread);
        let checking = !state.view.checked.is_empty();
        let words = state
            .view
            .search
            .as_deref()
            .map(app_front::highlight::words)
            .unwrap_or_default();
        let mut rows = Vec::new();
        for thread in ids
            .get(offset..end)
            .unwrap_or_default()
            .iter()
            .filter_map(|id| state.threads.get(id))
        {
            let summary = ThreadSummary::of(state, thread);
            let checked = state.view.checked.contains(&thread.id);
            let toggle = if checked {
                Command::UncheckThreads {
                    ids: vec![thread.id],
                }
            } else {
                Command::CheckThreads {
                    ids: vec![thread.id],
                }
            };
            let tap = if checking {
                toggle.clone()
            } else {
                Command::OpenThread { id: thread.id }
            };
            let action = self.add(Act::Run(tap));
            let long_press = self.add(Act::Run(toggle));
            let swipe_start = self.add(Act::Run(Command::ArchiveThreads {
                ids: vec![thread.id],
                messages: Vec::new(),
            }));
            let swipe_end = self.add(Act::Run(Command::TrashThreads {
                ids: vec![thread.id],
                messages: Vec::new(),
            }));
            let sender = format::names(&summary.senders);
            let snippet = summary
                .messages
                .last()
                .and_then(|m| m.snippet.as_deref())
                .unwrap_or_default()
                .trim();
            let subject = match thread.subject.trim() {
                "" => "(no subject)",
                s => s,
            };
            rows.push(Row {
                thread: thread.id,
                action,
                long_press,
                swipe_start,
                swipe_end,
                sender: if sender.is_empty() {
                    "(no sender)".to_owned()
                } else {
                    clean(&sender)
                },
                subject: clean(subject),
                snippet: clean(snippet),
                date: format::short_date(thread.last_date_ms, now),
                unread: summary.unread > 0,
                starred: summary.flagged,
                pinned: summary.pinned,
                attachments: summary.has_attachments,
                messages: summary.messages.len(),
                tag: (state.accounts.len() > 1 && tags > 0)
                    .then(|| state.accounts.get_index_of(&thread.account))
                    .flatten()
                    .map(|i| i % tags),
                selected: Some(thread.id) == cursor,
                checked,
                marks: sheet::row_marks(&words, subject, snippet),
                a11y: clean(&a11y::thread_row(&summary, checked, now)),
            });
        }
        let empty = (total == 0).then(|| {
            if state.view.search.is_some() {
                "No matching conversations."
            } else if state.view.mailbox.is_some() {
                "No conversations here."
            } else {
                "Pick a mailbox from the menu."
            }
            .to_owned()
        });
        List {
            title,
            total,
            offset,
            density,
            checked: state.view.checked.len(),
            refreshing: !status::can_sync(state),
            rows,
            server: sheet::server_state(state),
            empty,
        }
    }

    /// The bulk bar over `palette::targets` (the checked threads), while any are checked.
    fn bulk(&mut self, state: &AppState) -> Option<Bulk> {
        if state.view.checked.is_empty() {
            return None;
        }
        let ids = palette::targets(state);
        let messages = Vec::new;
        let all_pinned = ids
            .iter()
            .filter_map(|t| state.threads.get(t))
            .all(|t| listing::is_pinned(state, t));
        let actions = [
            (
                "archive",
                "Archive",
                Command::ArchiveThreads {
                    ids: ids.clone(),
                    messages: messages(),
                },
            ),
            (
                "delete",
                "Trash",
                Command::TrashThreads {
                    ids: ids.clone(),
                    messages: messages(),
                },
            ),
            (
                "mark_email_read",
                "Mark read",
                Command::MarkRead {
                    ids: ids.clone(),
                    messages: messages(),
                },
            ),
            (
                "mark_email_unread",
                "Mark unread",
                Command::MarkUnread {
                    ids: ids.clone(),
                    messages: messages(),
                },
            ),
            (
                "star",
                "Star",
                Command::Star {
                    ids: ids.clone(),
                    messages: messages(),
                },
            ),
            // MISC-4: unpin when every checked thread is pinned.
            if all_pinned {
                (
                    "push_pin",
                    "Unpin",
                    Command::UnpinThreads {
                        ids: ids.clone(),
                        messages: messages(),
                    },
                )
            } else {
                (
                    "push_pin",
                    "Pin",
                    Command::PinThreads {
                        ids: ids.clone(),
                        messages: messages(),
                    },
                )
            },
        ]
        .into_iter()
        .map(|(icon, label, cmd)| self.button(icon, label, Act::Run(cmd)))
        .collect();
        let close = self.button(
            "close",
            "Uncheck all",
            Act::Run(Command::UncheckThreads { ids: Vec::new() }),
        );
        Some(Bulk {
            title: format!("{} selected", ids.len()),
            actions,
            close,
        })
    }

    /// The open draft, with the typed text of the buffers.
    fn composer(&mut self, state: &AppState) -> Option<Composer> {
        let draft = self.compose.draft(state)?.clone();
        let now = self.now_ms();
        let editable = !draft.is_outgoing();
        let account = state.accounts.get(&draft.account);
        let from = draft.from.as_ref().map_or_else(
            || account.map_or_else(String::new, |a| a.email.clone()),
            format::full,
        );
        let sending_from = draft
            .from
            .as_ref()
            .map(|a| a.email.clone())
            .or_else(|| account.map(|a| a.email.clone()))
            .unwrap_or_default();
        let aliases: Vec<String> = account
            .map(|a| {
                std::iter::once(a.email.clone())
                    .chain(
                        a.identities
                            .iter()
                            .map(|i| i.email.clone())
                            .filter(|e| !e.eq_ignore_ascii_case(&a.email)),
                    )
                    .collect()
            })
            .unwrap_or_default();
        let mut from_choices = Vec::new();
        if aliases.len() > 1 && editable {
            for email in aliases {
                let selected = email.eq_ignore_ascii_case(&sending_from);
                let action = self.add(Act::Run(Command::SetDraftFrom {
                    id: draft.id,
                    email: email.clone(),
                }));
                from_choices.push(Choice {
                    label: clean(&email),
                    selected,
                    action,
                });
            }
        }
        let mut recipients = Vec::new();
        for (field, target) in [
            (Field::To, compose::Field::To),
            (Field::Cc, compose::Field::Cc),
            (Field::Bcc, compose::Field::Bcc),
        ] {
            let addrs = target.of(&draft);
            let mut chips = Vec::new();
            for (i, addr) in addrs.iter().enumerate() {
                let mut rest = addrs.to_vec();
                rest.remove(i);
                let remove = self.add(Act::Run(target.update(&draft, rest)));
                chips.push(Chip {
                    label: clean(&format::full(addr)),
                    remove,
                });
            }
            let mut suggestions = Vec::new();
            if editable {
                for addr in self.compose.suggestions(state, &draft, field) {
                    let label = clean(&format::full(&addr));
                    suggestions.push(Button {
                        icon: None,
                        label,
                        action: self.add(Act::Suggest(field, addr)),
                    });
                }
            }
            recipients.push(Recipients {
                field,
                label: target.label().to_owned(),
                chips,
                input: self.compose.value(field),
                suggestions,
            });
        }
        let quote = draft.quote.as_ref().map(|q| {
            let expanded = self.compose.quote_open;
            QuoteFold {
                label: match q.style {
                    QuoteStyle::Forward => "Forwarded message",
                    QuoteStyle::Reply => "Quoted message",
                }
                .to_owned(),
                header: if expanded {
                    clean_lines(q.header.trim_end())
                } else {
                    Vec::new()
                },
                expanded,
                toggle: self.add(Act::ToggleQuote),
            }
        });
        let mut problems = Vec::new();
        if let Some(error) = &draft.send_error {
            problems.push(format!("Not sent: {}", clean(error)));
            if let Some(hint) = status::send_hint(state, draft.account, error, self.lang) {
                problems.push(clean(&hint));
            }
        }
        if let Some(error) = &draft.save_error {
            problems.push(format!("Draft save failed: {}", clean(error)));
        }
        let mut attachments = Vec::new();
        for (i, file) in draft.attachments.iter().enumerate() {
            let (detail, failed) = match &file.state {
                AttachmentState::Checking => ("checking…".to_owned(), false),
                AttachmentState::Ready { size, .. } => (format::size(*size), false),
                AttachmentState::Failed { error } => (clean(error), true),
            };
            let remove = editable
                .then(|| u32::try_from(i).ok())
                .flatten()
                .map(|index| {
                    self.add(Act::Run(Command::RemoveAttachment {
                        draft: draft.id,
                        index,
                    }))
                });
            attachments.push(OutgoingFile {
                name: clean(&file.name),
                detail,
                failed,
                remove,
            });
        }
        let mut actions = Vec::new();
        if editable && compose::sendable(&draft) {
            actions.push(self.button(
                "send",
                "Send",
                Act::Run(Command::SendDraft { id: draft.id }),
            ));
        }
        if editable {
            actions.push(self.button("attach_file", "Attach files", Act::Pick(draft.id)));
            actions.push(self.button(
                "delete",
                "Discard",
                Act::Run(Command::DiscardDraft { id: draft.id }),
            ));
        }
        Some(Composer {
            draft: draft.id,
            title: compose::title(&draft).to_owned(),
            status: compose::draft_status(state, &draft, now),
            editable,
            from: clean(&from),
            from_choices,
            recipients,
            subject: self.compose.value(Field::Subject),
            body: self.compose.value(Field::Body),
            quote,
            attachments,
            problems,
            actions,
        })
    }

    /// `External::Schedule` with the first frame and when the background settings change.
    fn tell_schedule(&mut self, state: &AppState) {
        let now = (
            state.settings.background_sync,
            state.settings.background_interval_mins,
        );
        if self.schedule != Some(now) {
            self.schedule = Some(now);
            self.externals.push(External::Schedule {
                sync: now.0,
                interval_mins: now.1,
            });
        }
    }

    /// `External::Sending` once per send that is scheduled or leaving, from `send_draft` or the
    /// undo of `cancel_send`, whoever scheduled it.
    fn keep_sends(&mut self, state: &AppState) {
        let outgoing = |id: &DraftId| state.drafts.get(id).is_some_and(Draft::is_outgoing);
        self.kept.retain(|id| outgoing(id));
        for draft in state.drafts.values().filter(|d| d.is_outgoing()) {
            if !self.kept.contains(&draft.id) {
                self.kept.push(draft.id);
                self.externals.push(External::Sending { draft: draft.id });
            }
        }
    }

    /// *Not sent* once per send error (a failed send, or one a restart cancelled), until the
    /// draft's error changes.
    fn tell_send_errors(&mut self, state: &AppState) {
        self.told.retain(|(id, error)| {
            state
                .drafts
                .get(id)
                .is_some_and(|d| d.send_error.as_ref() == Some(error))
        });
        for draft in state.drafts.values() {
            let Some(error) = &draft.send_error else {
                continue;
            };
            if self.told.iter().any(|(id, _)| *id == draft.id) {
                continue;
            }
            self.told.push((draft.id, error.clone()));
            let subject = match draft.subject.trim() {
                "" => "(no subject)",
                s => s,
            };
            self.notify(&format!("Not sent: “{subject}”: {error}"));
        }
    }

    fn banner(&mut self, state: &AppState) -> Option<Banner> {
        let Some(problem) = status::problems(state, self.lang).into_iter().next() else {
            return self.update_banner(state);
        };
        let action = problem.fix.map(|fix| {
            let act = match fix {
                Fix::Retry(account) => Act::Run(Command::SyncNow {
                    account: Some(account),
                }),
                Fix::Test(id) => Act::Run(Command::TestAccount { id }),
                Fix::Password(account) => Act::Reenter(account),
            };
            Button {
                icon: None,
                label: fix.label(self.lang),
                action: self.add(act),
            }
        });
        Some(Banner {
            text: clean(&problem.text),
            hint: problem.hint.map(|h| clean(&h)),
            action,
            dismiss: None,
            info: false,
        })
    }

    /// Wave 7 F2/F3: the first-run question about update checks, else a new version with its
    /// release page (the apps do not install: the user updates from there, or Obtainium does).
    fn update_banner(&mut self, state: &AppState) -> Option<Banner> {
        if updates::ask_first_run(state) {
            let yes = self.add(Act::Run(updates::answer(true)));
            let no = self.add(Act::Run(updates::answer(false)));
            return Some(Banner {
                text: updates::QUESTION.to_owned(),
                hint: Some(updates::QUESTION_DETAIL.to_owned()),
                action: Some(Button {
                    icon: None,
                    label: "Check".to_owned(),
                    action: yes,
                }),
                dismiss: Some(Button {
                    icon: None,
                    label: "No thanks".to_owned(),
                    action: no,
                }),
                info: true,
            });
        }
        let release = updates::pending(state)?;
        let open = self.add(Act::OpenLink(release.notes_url.clone()));
        let skip = self.add(Act::Run(Command::SkipUpdateVersion {
            version: Some(release.version.clone()),
        }));
        Some(Banner {
            text: format!("dail {} is available", clean(&release.version)),
            hint: None,
            action: Some(Button {
                icon: None,
                label: "Open".to_owned(),
                action: open,
            }),
            dismiss: Some(Button {
                icon: None,
                label: "Skip".to_owned(),
                action: skip,
            }),
            info: true,
        })
    }

    fn snackbar(&mut self, state: &AppState) -> Option<Snackbar> {
        let now = self.now_ms();
        if let Some(draft) = self.sending.filter(|id| notice::send_pending(state, *id)) {
            let text = notice::sending_text(state, draft, now);
            let action = self.add(Act::Run(Command::CancelSend { id: draft }));
            return Some(Snackbar {
                text,
                action: Some(Button {
                    icon: None,
                    label: "Undo".to_owned(),
                    action,
                }),
            });
        }
        if let Some((text, until)) = &self.notice
            && *until > now
        {
            return Some(Snackbar {
                text: clean(text),
                action: None,
            });
        }
        if self.undo_until > now
            && let Some(label) = self.last.label().map(str::to_owned)
        {
            let action = self.add(Act::Run(Command::Undo {}));
            return Some(Snackbar {
                text: clean(&label),
                action: Some(Button {
                    icon: None,
                    label: "Undo".to_owned(),
                    action,
                }),
            });
        }
        None
    }
}

/// The application ID; content URIs of its providers are its own files.
const APP_ID: &str = "net.darii.dail";

/// Whether a shared URI may be attached: a `content://` URI of another app. `file://` would
/// let a sender name any file dail can read, and dail's own providers serve its private files.
fn shareable(uri: &str) -> bool {
    let Some(rest) = uri
        .get(..10)
        .filter(|s| s.eq_ignore_ascii_case("content://"))
        .and_then(|_| uri.get(10..))
    else {
        return false;
    };
    // `user@` before the host names another user's provider (`content://10@…`).
    let authority = rest.split('/').next().unwrap_or_default();
    let authority = authority
        .rsplit('@')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    !authority.is_empty() && authority != APP_ID && !authority.starts_with(&format!("{APP_ID}."))
}

/// The list's title: the search, or the open mailbox.
fn list_title(state: &AppState) -> String {
    if let Some(search) = &state.view.search {
        return format!("Search: {}", clean(search));
    }
    if state.view.unified {
        return "All inboxes".to_owned();
    }
    state
        .view
        .mailbox
        .and_then(|id| state.mailboxes.get(&id))
        .map_or_else(|| "Conversations".to_owned(), |m| clean(&mailbox_name(m)))
}
