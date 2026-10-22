//! `Mobile`: the presenter's UI-local state, and the projection of `AppState` into a [`Frame`].
//!
//! Navigation is the `ViewState`, as in the TUI: the drawer is `focus_pane sidebar`, a thread is
//! `open_thread`, and Back is the `Escape` binding. The window class only chooses panes
//! (`app_front::panes`); it never dispatches a command.

use app_core::{
    AppState, BodyState, Command, CommandName, DraftId, MailboxRole, Pane, ThreadId, ThreadSummary,
    listing,
};
use app_front::body::Bodies;
use app_front::i18n::Lang;
use app_front::keymap::{Action, Chord, Key, KeyState, Press, parse_chord};
use app_front::mailbox::mailbox_name;
use app_front::notice::{self, LastAction};
use app_front::panes::{self, Slot, Width};
use app_front::status::{self, Fix};
use app_front::text::{clean, clean_lines};
use app_front::{a11y, body, fold, format, palette};

use crate::event::{External, UiEvent};
use crate::frame::{
    ActionId, Attachment, Banner, Button, Class, Composer, Density, Drawer, DrawerAccount,
    DrawerItem, Frame, Layout, List, PaneKind, Reader, ReaderMessage, Row, Snackbar, TopBar,
};
use crate::request::{Purpose, Reply, Request};

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
    /// What Back does.
    Back,
    /// Start a new message from the current account (`create_draft`, then the composer).
    Compose,
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

    /// What only the host can do, asked for since the last call.
    pub fn externals(&mut self) -> Vec<External> {
        std::mem::take(&mut self.externals)
    }

    /// Turns one event into what to submit.
    pub fn event(&mut self, event: UiEvent, state: &AppState) -> Vec<Request> {
        match event {
            UiEvent::Configure {
                width_dp,
                height_dp,
                locale,
                ..
            } => {
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
            UiEvent::Tap { action } | UiEvent::LongPress { action } => self.act(action, state),
            UiEvent::Swipe { action } => {
                let out = self.act(action, state);
                if !out.is_empty() {
                    self.externals.push(External::Haptic);
                }
                out
            }
            UiEvent::Scrolled { first, count } => {
                self.scrolled = (first, count);
                Vec::new()
            }
            UiEvent::Back => self.back(state),
            UiEvent::Refresh => vec![Request::user(Command::SyncNow { account: None })],
            UiEvent::Key { chord } => match parse_chord(&chord) {
                Some(chord) => self.press(chord, state),
                None => Vec::new(),
            },
            // Text fields arrive with the composer (Wave 4 A4); the clock only redraws.
            UiEvent::Text { .. } | UiEvent::Blur { .. } | UiEvent::Tick => Vec::new(),
        }
    }

    /// Takes the results of earlier requests; returns what they lead to (a new draft opens in
    /// the composer, as in the GUI).
    pub fn replies(&mut self, replies: Vec<Reply>, _state: &AppState) -> Vec<Request> {
        let mut out = Vec::new();
        let now = self.now_ms();
        for reply in replies {
            match reply.purpose {
                Purpose::Bodies(thread) => {
                    let receipt = reply.result.as_ref().ok().and_then(|r| r.as_ref().ok());
                    self.bodies.accept(thread, receipt);
                }
                Purpose::User => match &reply.result {
                    Ok(Ok(receipt)) => {
                        let before = self.last.label().map(str::to_owned);
                        if let Some(draft) = self.last.observe(receipt) {
                            self.sending = Some(draft);
                        }
                        if self.last.label().is_some() && self.last.label() != before.as_deref() {
                            self.undo_until = now + NOTICE_MS;
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
        match act {
            Act::Run(cmd) => vec![Request::user(cmd)],
            Act::Back => self.back(state),
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

    fn press(&mut self, chord: Chord, state: &AppState) -> Vec<Request> {
        let now = self.now_ms();
        match self.keys.press(chord, false, state, now) {
            Press::Done(Action::Run(cmd)) => vec![Request::user(cmd)],
            // The palette's steps, the wizard and the password form arrive with their items.
            Press::Done(_) | Press::Pending | Press::Unbound => Vec::new(),
        }
    }

    /// What Back submits: the `Escape` binding (close the composer, the thread, the search, …),
    /// else uncheck the checked threads, else from the drawer of a one-pane window back to the list.
    fn back(&mut self, state: &AppState) -> Vec<Request> {
        if let Some(cmd) = self.escape(state) {
            return vec![Request::user(cmd)];
        }
        if !state.view.checked.is_empty() {
            return vec![Request::user(Command::UncheckThreads { ids: Vec::new() })];
        }
        if self.drawer_back(state) {
            return vec![Request::user(Command::FocusPane { pane: Pane::List })];
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

    fn drawer_back(&self, state: &AppState) -> bool {
        self.class() != Class::Expanded && state.view.focus == Pane::Sidebar
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
        self.actions.clear();
        self.revision += 1;
        let class = self.class();
        let width = match class {
            Class::Compact => Width::Narrow,
            Class::Medium => Width::Medium,
            Class::Expanded => Width::Wide,
        };
        let slots = if state.accounts.is_empty() {
            vec![Slot::List]
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
        // A collapsed sidebar on a wide window is a rail, not gone.
        let rail =
            class == Class::Expanded && state.view.sidebar_collapsed && !state.accounts.is_empty();
        if rail {
            kinds.insert(0, PaneKind::Drawer);
        }
        let top_bar = self.top_bar(state, &kinds);
        let drawer = kinds
            .contains(&PaneKind::Drawer)
            .then(|| self.drawer(state, rail));
        let list = kinds.contains(&PaneKind::List).then(|| self.list(state));
        let reader = if kinds.contains(&PaneKind::Reader) {
            self.reader(state)
        } else {
            None
        };
        let composer = if kinds.contains(&PaneKind::Composer) {
            composer(state)
        } else {
            None
        };
        let fab = (!state.accounts.is_empty() && !kinds.contains(&PaneKind::Composer))
            .then(|| self.button("edit", "Compose", Act::Compose));
        let banner = self.banner(state);
        let snackbar = self.snackbar(state);
        let back = self.escape(state).is_some()
            || !state.view.checked.is_empty()
            || self.drawer_back(state);
        Frame {
            revision: self.revision,
            layout: Layout {
                class,
                panes: kinds,
            },
            top_bar,
            drawer,
            list,
            reader,
            composer,
            fab,
            banner,
            snackbar,
            back,
            wants_tick: self.wants_tick(state),
        }
    }

    fn top_bar(&mut self, state: &AppState, kinds: &[PaneKind]) -> TopBar {
        let one = match kinds {
            [only] => Some(*only),
            _ => None,
        };
        let title = match one {
            Some(PaneKind::Drawer) => "Mailboxes".to_owned(),
            Some(PaneKind::Reader) => "Message".to_owned(),
            Some(PaneKind::Composer) => "Draft".to_owned(),
            Some(PaneKind::List) | None => list_title(state),
        };
        let subtitle = (!state.accounts.is_empty())
            .then(|| status::sync_text(state, self.now_ms(), self.lang))
            .filter(|s| !s.is_empty());
        let navigation = match one {
            _ if state.accounts.is_empty() => None,
            Some(PaneKind::Reader | PaneKind::Composer | PaneKind::Drawer) => {
                Some(self.button("back", "Back", Act::Back))
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
        TopBar {
            title,
            subtitle,
            navigation,
            actions,
        }
    }

    fn drawer(&mut self, state: &AppState, rail: bool) -> Drawer {
        let mut accounts = Vec::new();
        for account in state.accounts.values() {
            let mut mailboxes = Vec::new();
            for mailbox in listing::mailboxes(state, Some(account.id)) {
                let label = clean(&mailbox_name(mailbox));
                let action = self.add(Act::Run(Command::OpenMailbox { id: mailbox.id }));
                mailboxes.push(DrawerItem {
                    a11y: a11y::nav_row(&label, mailbox.unread),
                    label,
                    icon: mailbox_icon(mailbox.role).to_owned(),
                    unread: mailbox.unread,
                    selected: state.view.mailbox == Some(mailbox.id) && state.view.search.is_none(),
                    action,
                });
            }
            accounts.push(DrawerAccount {
                name: clean(&account.display_name),
                email: clean(&account.email),
                mailboxes,
            });
        }
        Drawer { rail, accounts }
    }

    fn list(&mut self, state: &AppState) -> List {
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
                rows: Vec::new(),
                empty: Some(
                    "No accounts yet. Account setup arrives in a later version.".to_owned(),
                ),
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
                .unwrap_or_default();
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
                subject: match thread.subject.trim() {
                    "" => "(no subject)".to_owned(),
                    s => clean(s),
                },
                snippet: clean(snippet.trim()),
                date: format::short_date(thread.last_date_ms, now),
                unread: summary.unread > 0,
                starred: summary.flagged,
                attachments: summary.has_attachments,
                messages: summary.messages.len(),
                selected: Some(thread.id) == cursor,
                checked,
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
            rows,
            empty,
        }
    }

    fn reader(&mut self, state: &AppState) -> Option<Reader> {
        let thread = state.view.thread.and_then(|id| state.threads.get(&id))?;
        let now = self.now_ms();
        let ids = vec![thread.id];
        let mut actions = vec![
            self.button(
                "archive",
                "Archive",
                Act::Run(Command::ArchiveThreads {
                    ids: ids.clone(),
                    messages: Vec::new(),
                }),
            ),
            self.button(
                "delete",
                "Trash",
                Act::Run(Command::TrashThreads {
                    ids: ids.clone(),
                    messages: Vec::new(),
                }),
            ),
            self.button(
                "mark_email_unread",
                "Mark unread",
                Act::Run(Command::MarkUnread {
                    ids,
                    messages: Vec::new(),
                }),
            ),
        ];
        if let Some(message) = palette::newest_message(state, thread.id) {
            actions.push(self.button(
                "reply",
                "Reply",
                Act::Run(Command::Reply {
                    message,
                    body: None,
                }),
            ));
        }
        let mut messages = Vec::new();
        for message in thread
            .messages
            .iter()
            .filter_map(|id| state.messages.get(id))
        {
            let expanded = listing::is_expanded(state, thread, message);
            let toggle = self.add(Act::Run(if expanded {
                Command::CollapseMessages {
                    ids: vec![message.id],
                }
            } else {
                Command::ExpandMessages {
                    ids: vec![message.id],
                }
            }));
            let loaded = self.bodies.get(message.id);
            let snippet = loaded
                .map(|b| fold::split(&b.text).main)
                .and_then(|text| text.lines().find(|l| !l.trim().is_empty()))
                .or(message.snippet.as_deref())
                .unwrap_or_default()
                .trim()
                .chars()
                .take(120)
                .collect::<String>();
            let (lines, status) = match (expanded, loaded, &message.body) {
                (false, _, _) => (None, None),
                (true, Some(body), _) => (Some(clean_lines(body.text.trim_end())), None),
                (true, None, BodyState::Failed { error }) => (
                    None,
                    Some(format!(
                        "The body could not be downloaded: {}",
                        clean(error)
                    )),
                ),
                (true, None, _) => (None, Some("Loading…".to_owned())),
            };
            let to: Vec<String> = message
                .to
                .iter()
                .chain(&message.cc)
                .map(|a| clean(&format::full(a)))
                .collect();
            messages.push(ReaderMessage {
                message: message.id,
                from: message
                    .from
                    .first()
                    .map_or_else(|| "(no sender)".to_owned(), |a| clean(&format::full(a))),
                to: to.join(", "),
                date: format::short_date(message.date_ms, now),
                expanded,
                toggle,
                snippet: clean(&snippet),
                body: lines,
                status,
                attachments: loaded
                    .map(|b| {
                        b.attachments
                            .iter()
                            .map(|a| Attachment {
                                part: a.part,
                                name: clean(&a.name),
                                mime: clean(&a.content_type),
                                size: format::size(a.size),
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                links: loaded
                    .map(|b| body::links(&b.text).iter().map(|l| clean(l)).collect())
                    .unwrap_or_default(),
            });
        }
        Some(Reader {
            thread: thread.id,
            subject: match thread.subject.trim() {
                "" => "(no subject)".to_owned(),
                s => clean(s),
            },
            actions,
            messages,
        })
    }

    fn banner(&mut self, state: &AppState) -> Option<Banner> {
        let problem = status::problems(state, self.lang).into_iter().next()?;
        let action = problem.fix.and_then(|fix| {
            let cmd = match fix {
                Fix::Retry(account) => Command::SyncNow {
                    account: Some(account),
                },
                Fix::Test(id) => Command::TestAccount { id },
                // The password form arrives with account setup (Wave 4 A6).
                Fix::Password(_) => return None,
            };
            Some(Button {
                icon: None,
                label: fix.label(self.lang),
                action: self.add(Act::Run(cmd)),
            })
        });
        Some(Banner {
            text: clean(&problem.text),
            hint: problem.hint.map(|h| clean(&h)),
            action,
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

fn composer(state: &AppState) -> Option<Composer> {
    let draft = state.view.composer.and_then(|id| state.drafts.get(&id))?;
    let list = |addrs: &[app_core::Addr]| {
        addrs
            .iter()
            .map(|a| clean(&format::full(a)))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let from = draft
        .from
        .as_ref()
        .map(format::full)
        .or_else(|| state.accounts.get(&draft.account).map(|a| a.email.clone()))
        .unwrap_or_default();
    Some(Composer {
        draft: draft.id,
        from: clean(&from),
        to: list(&draft.to),
        cc: list(&draft.cc),
        subject: clean(&draft.subject),
        body: draft.body_text.clone(),
    })
}
