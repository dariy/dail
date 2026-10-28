//! `Tui`: one frame of the terminal UI, as a pure function of `AppState` plus UI-local state.
//!
//! The TUI reads `&AppState` and never changes it. Everything the user does becomes a
//! [`Request`] returned by [`Tui::input`]; the runner hands them to the session. A resize never
//! dispatches a command: the view is shared with the GUI and with agents.

use app_core::{AppState, Command, CommandName, DraftId, Pane, ThreadSummary, listing};
use app_front::body::Bodies;
use app_front::keymap::{Action, Chord, Key, KeyState, Press};
use app_front::notice::{self, LastAction};

use ratatui::Frame;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::approvals::{self, ApprovalsView};
use crate::composer::ComposerView;
use crate::layout::{self, Slot};
use crate::palette::{PaletteView, ResultView, overlay_area};
use crate::reader::Reader;
use crate::request::{Purpose, Reply, Request};
use crate::setup::Setup;
use crate::{keys, view};

/// One input event, in the order the terminal delivered it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// A key press.
    Key(KeyEvent),
    /// Text pasted with bracketed paste.
    Paste(String),
    /// The terminal was resized (columns, rows).
    Resize(u16, u16),
    /// The clock moved on (countdowns and ages are redrawn).
    Tick,
    /// A mouse event (a tap or a swipe on a phone), while mouse capture is on.
    Mouse(MouseEvent),
}

/// What a spot of the last frame shows, for taps and the wheel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hit {
    /// A row of the thread list.
    Thread(app_core::ThreadId),
    /// A mailbox in the mailboxes pane.
    Mailbox(app_core::MailboxId),
    /// The list pane.
    List,
    /// The reader pane.
    Reader,
}

/// Something the user asked for that happens outside the TUI: the runner performs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum External {
    /// Turn the terminal's mouse capture on or off (off: the terminal's own selection works,
    /// e.g. to copy a link).
    MouseCapture(bool),
    /// Edit a draft's body in `$VISUAL`/`$EDITOR`, starting from `text`.
    EditBody {
        /// The draft.
        draft: DraftId,
        /// The body as typed so far.
        text: String,
    },
}

/// How an external edit went (the runner's report).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditOutcome {
    /// The editor saved this text.
    Text(String),
    /// The editor failed; the body is unchanged.
    Failed(String),
    /// Neither `$VISUAL` nor `$EDITOR` is set.
    NoEditor,
}

/// UI-local state that is not part of `AppState`.
pub struct Tui {
    clock: fn() -> i64,
    /// The mark-as-read timer (UX-5).
    autoread: app_front::autoread::AutoRead,
    /// `NO_COLOR` is set: never paint the theme's colours.
    no_color: bool,
    /// The language of the shared texts (`app_front::i18n`).
    lang: app_front::i18n::Lang,
    quit: bool,
    setup_hint: String,
    /// The first visible row of the thread list.
    list_offset: usize,
    /// The list's cursor in the last frame: the list follows the cursor only when it moved.
    list_cursor: Option<app_core::ThreadId>,
    /// What each spot of the last frame shows.
    hits: Vec<(Rect, Hit)>,
    /// Whether the terminal's mouse capture is on.
    mouse: bool,
    bodies: Bodies,
    reader: Reader,
    /// The panes of the last frame.
    slots: Vec<Slot>,
    keys: KeyState,
    palette: PaletteView,
    /// The last query result the user asked for.
    result: Option<ResultView>,
    /// The agents' send requests, while the user looks at them.
    approvals: Option<ApprovalsView>,
    /// The settings view's cursor (shown while `view.settings`).
    settings: crate::settings::SettingsView,
    composer: ComposerView,
    /// The account setup, while it is open.
    setup: Option<Setup>,
    external: Option<External>,
    /// Whether this TUI owns the session (closing it cancels a scheduled send).
    owns_session: bool,
    /// Quitting was asked for while a send is scheduled: the next `q` or Ctrl+C quits.
    confirm_quit: bool,
    /// A short message for the status line, until the given time (Unix ms).
    notice: Option<(String, i64)>,
    /// The latest undoable action of this TUI.
    last: LastAction,
    /// The draft whose send this TUI scheduled, while it can be cancelled.
    sending: Option<DraftId>,
}

impl Default for Tui {
    fn default() -> Self {
        Self {
            clock: system_now_ms,
            autoread: app_front::autoread::AutoRead::default(),
            no_color: false,
            lang: app_front::i18n::Lang::default(),
            quit: false,
            setup_hint: "dail account add --preset gmail --email you@gmail.com".to_owned(),
            list_offset: 0,
            list_cursor: None,
            hits: Vec::new(),
            mouse: true,
            bodies: Bodies::default(),
            reader: Reader::default(),
            slots: Vec::new(),
            keys: KeyState::default(),
            palette: PaletteView::default(),
            result: None,
            approvals: None,
            settings: crate::settings::SettingsView::default(),
            composer: ComposerView::default(),
            setup: None,
            external: None,
            owns_session: false,
            confirm_quit: false,
            notice: None,
            last: LastAction::default(),
            sending: None,
        }
    }
}

impl std::fmt::Debug for Tui {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tui")
            .field("quit", &self.quit)
            .finish_non_exhaustive()
    }
}

/// The question before quitting an owned session while a send waits.
const QUIT_PROMPT: &str =
    "Quitting cancels the send still in its undo window. q quits, any other key stays";

/// How long a notice stays in the status line.
const NOTICE_MS: i64 = 6_000;

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

fn system_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

impl Tui {
    /// Shows the shared texts in the language of `tag` (`$DAIL_LANG`, `app_front::i18n`).
    #[must_use]
    pub fn with_lang_tag(mut self, tag: Option<&str>) -> Self {
        let lang = app_front::i18n::Lang::from_tag(tag);
        self.lang = lang;
        self.composer.lang = lang;
        self
    }

    /// A TUI that reads the time from `clock` (Unix ms) instead of the system clock.
    pub fn with_clock(clock: fn() -> i64) -> Self {
        Self {
            clock,
            ..Self::default()
        }
    }

    /// Tells the TUI that it owns the session, so quitting while a send is scheduled asks first
    /// (closing inside the undo window cancels the send, `ARCHITECTURE.md §4.4`).
    #[must_use]
    pub const fn owning_session(mut self, owns: bool) -> Self {
        self.owns_session = owns;
        self
    }

    /// The command line shown while there are no accounts, e.g. with the profile's flags.
    #[must_use]
    pub fn with_setup_hint(mut self, hint: impl Into<String>) -> Self {
        self.setup_hint = hint.into();
        self
    }

    /// The time the TUI shows (Unix ms).
    pub fn now_ms(&self) -> i64 {
        (self.clock)()
    }

    /// Whether the user asked to quit.
    pub const fn quit_requested(&self) -> bool {
        self.quit
    }

    /// Whether the screen changes with the clock: a send counts down, or an age (`synced 2m
    /// ago`) is shown. The runner then ticks every second.
    pub fn wants_tick(&self, state: &AppState) -> bool {
        self.composer.is_dirty()
            || !state.accounts.is_empty()
            || state.drafts.values().any(|d| d.send_at.is_some())
            || self.notice.is_some()
    }

    /// Draws one frame of `state`, in the theme's colours when `tui_colors` asks for them and
    /// `NO_COLOR` is unset (UI-16).
    pub fn draw(&mut self, frame: &mut Frame, state: &AppState) {
        self.draw_widgets(frame, state);
        if state.settings.tui_colors == 1 && !self.no_color {
            crate::colors::paint(
                frame.buffer_mut(),
                &crate::colors::theme_of(&state.settings),
            );
        }
    }

    /// Tells the TUI that `NO_COLOR` is set: it keeps the terminal's colours whatever
    /// `tui_colors` says.
    pub fn with_no_color(mut self, no_color: bool) -> Self {
        self.no_color = no_color;
        self
    }

    fn draw_widgets(&mut self, frame: &mut Frame, state: &AppState) {
        let total = frame.area();
        let banner = approvals::banner(state);
        let [header, banner_area, body, status] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(u16::from(banner.is_some())),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .areas(total);
        let Some(arrangement) = layout::arrange(total, body, state) else {
            view::too_small(frame, total);
            return;
        };
        self.slots = arrangement.panes.iter().map(|(slot, _)| *slot).collect();
        self.hits.clear();
        let one = match arrangement.panes.as_slice() {
            [(slot, _)] => Some(*slot),
            _ => None,
        };
        frame.render_widget(Paragraph::new(view::header(state, one)), header);
        if let Some(banner) = banner {
            frame.render_widget(Paragraph::new(banner), banner_area);
        }
        if state.accounts.is_empty() {
            frame.render_widget(self.setup(), body);
        } else {
            for (slot, area) in &arrangement.panes {
                self.draw_pane(frame, *slot, *area, arrangement.borders, state);
            }
        }
        let borders = arrangement.borders;
        if let Some(view) = &mut self.approvals {
            view.draw(frame, overlay_area(body, borders, 90), state, borders);
        }
        if state.view.settings.is_some() {
            self.settings
                .draw(frame, overlay_area(body, borders, 90), state, borders);
        }
        if let Some(setup) = &self.setup {
            setup.draw(frame, overlay_area(body, borders, 72), state, borders);
        }
        if let Some(result) = &mut self.result {
            result.draw(frame, overlay_area(body, borders, 90), borders);
        }
        if state.view.palette_open {
            self.palette
                .draw(frame, overlay_area(body, borders, 72), state, borders);
        }
        let now = self.now_ms();
        let sending = self
            .sending
            .filter(|id| notice::send_pending(state, *id))
            .map(|id| notice::sending_text(state, id, now));
        let prompt = self.confirm_quit.then_some(QUIT_PROMPT);
        let parts = view::StatusParts {
            prompt,
            sending,
            notice: self.notice(),
            undo: self.last.label(),
            lang: self.lang,
        };
        let line = view::status(state, now, &parts, status.width);
        frame.render_widget(Paragraph::new(line), status);
    }

    fn draw_pane(
        &mut self,
        frame: &mut Frame,
        slot: Slot,
        area: Rect,
        borders: bool,
        state: &AppState,
    ) {
        let focused = state.view.focus == slot.pane();
        let block = view::pane_block(view::slot_title(slot, state), focused, borders);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        match slot {
            Slot::List => self.hits.push((inner, Hit::List)),
            Slot::Reader => self.hits.push((inner, Hit::Reader)),
            Slot::Mailboxes | Slot::Composer => {}
        }
        let lines = match slot {
            Slot::Mailboxes => {
                let rows = view::mailboxes(state);
                for (y, (_, id)) in (inner.y..inner.y.saturating_add(inner.height)).zip(&rows) {
                    if let Some(id) = id {
                        let row = Rect {
                            y,
                            height: 1,
                            ..inner
                        };
                        self.hits.push((row, Hit::Mailbox(*id)));
                    }
                }
                rows.into_iter().map(|(line, _)| line).collect()
            }
            Slot::List => self.list_lines(state, inner),
            Slot::Reader => {
                let now = self.now_ms();
                self.reader
                    .lines(state, &self.bodies, now, inner.width, inner.height)
            }
            Slot::Composer => {
                let now = self.now_ms();
                self.composer.draw(frame, inner, state, now, focused);
                return;
            }
        };
        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn list_lines(&mut self, state: &AppState, area: Rect) -> Vec<Line<'static>> {
        let rows = listing::visible_threads(state);
        if rows.is_empty() {
            let text = if state.view.search.is_some() {
                "No matching conversations."
            } else if state.view.mailbox.is_some() {
                "No conversations here."
            } else {
                "Pick a mailbox: g i for the inbox."
            };
            return vec![Line::from(Span::styled(text, view::dim()))];
        }
        // The cursor is the selection, as in the GUI (the open thread if nothing is selected).
        let cursor = state.view.selected.last().copied().or(state.view.thread);
        let at = cursor.and_then(|c| rows.iter().position(|r| *r == c));
        let height = usize::from(area.height);
        if cursor == self.list_cursor {
            // Scrolled by the wheel: stay, within the list.
            self.list_offset = self.list_offset.min(rows.len().saturating_sub(height));
        } else {
            self.list_offset = view::scroll_to(self.list_offset, at, height, rows.len());
            self.list_cursor = cursor;
        }
        let now = self.now_ms();
        let visible: Vec<&app_core::Thread> = rows
            .iter()
            .skip(self.list_offset)
            .take(height)
            .filter_map(|id| state.threads.get(id))
            .collect();
        for (y, thread) in (area.y..).zip(&visible) {
            let row = Rect {
                y,
                height: 1,
                ..area
            };
            self.hits.push((row, Hit::Thread(thread.id)));
        }
        visible
            .into_iter()
            .map(|thread| {
                let summary = ThreadSummary::of(state, thread);
                let marks = (
                    Some(thread.id) == cursor,
                    state.view.checked.contains(&thread.id),
                );
                view::thread_row(&summary, now, area.width, marks)
            })
            .collect()
    }

    /// The first-run screen: how to add an account from another shell (it then appears live).
    fn setup(&self) -> Paragraph<'static> {
        Paragraph::new(vec![
            Line::from(Span::styled(
                "No accounts yet.",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("Press Enter to add one here, or run in another shell:"),
            Line::from(""),
            Line::from(format!("  {}", self.setup_hint)),
            Line::from(""),
            Line::from("The account shows up here once it is added."),
        ])
        .wrap(ratatui::widgets::Wrap { trim: false })
    }

    /// The background requests the current state calls for (the reader's bodies). The runner
    /// calls it before each frame.
    pub fn refresh(&mut self, state: &AppState) -> Vec<Request> {
        self.palette.sync(state);
        if state.approvals.is_empty() {
            self.approvals = None;
        }
        self.composer.sync(state);
        let now = self.now_ms();
        let idle = self.composer.idle(state, now);
        // Mark as read after a delay (UX-5); a terminal has no focus signal: always counts.
        let read = match self.autoread.poll(state, now, true) {
            app_front::autoread::Poll::Fire(cmd) => Some(Request::user(cmd)),
            app_front::autoread::Poll::Wait(_) | app_front::autoread::Poll::Idle => None,
        };
        idle.into_iter()
            .chain(self.bodies_request(state))
            .chain(read)
            .collect()
    }

    fn bodies_request(&mut self, state: &AppState) -> Vec<Request> {
        self.bodies
            .refresh(state)
            .map(|(thread, cmd)| Request {
                purpose: Purpose::Bodies(thread),
                cmd,
            })
            .into_iter()
            .collect()
    }

    /// Whether the reader is the only pane or has the focus (↑/↓ scroll it then).
    fn reader_active(&self, state: &AppState) -> bool {
        self.slots.contains(&Slot::Reader)
            && (self.slots.len() == 1 || state.view.focus == app_core::Pane::Reader)
    }

    /// What the user asked for outside the TUI since the last call.
    pub fn take_external(&mut self) -> Option<External> {
        self.external.take()
    }

    /// The runner's report on an external edit of `draft`'s body.
    pub fn edited(
        &mut self,
        draft: DraftId,
        outcome: EditOutcome,
        state: &AppState,
    ) -> Vec<Request> {
        match outcome {
            EditOutcome::Text(text) => {
                let text = text.strip_suffix('\n').unwrap_or(&text).to_owned();
                match state.drafts.get(&draft) {
                    Some(d) if d.body_text != text => vec![Request::user(Command::UpdateDraft {
                        id: draft,
                        to: None,
                        cc: None,
                        bcc: None,
                        subject: None,
                        body: Some(text),
                    })],
                    _ => Vec::new(),
                }
            }
            EditOutcome::Failed(reason) => {
                self.notify(format!("The editor failed: {reason}"));
                Vec::new()
            }
            EditOutcome::NoEditor => {
                self.notify("Set $VISUAL or $EDITOR to write the body in your editor");
                Vec::new()
            }
        }
    }

    /// Whether the account setup is open.
    pub const fn setup_open(&self) -> bool {
        self.setup.is_some()
    }

    /// The palette's current second step, if it is at one.
    pub fn palette_step(&self) -> Option<&app_front::palette::Ask> {
        self.palette.step()
    }

    /// Shows `text` in the status line for a few seconds.
    pub fn notify(&mut self, text: impl Into<String>) {
        self.notice = Some((text.into(), self.now_ms() + NOTICE_MS));
    }

    /// The status line's notice, while it lasts.
    pub fn notice(&self) -> Option<&str> {
        self.notice
            .as_ref()
            .filter(|(_, until)| *until > self.now_ms())
            .map(|(text, _)| text.as_str())
    }

    /// Handles one input event and returns the commands it issued, in order.
    pub fn input(&mut self, input: Input, state: &AppState) -> Vec<Request> {
        let requests = self.input_inner(input, state);
        for request in &requests {
            self.autoread.note(&request.cmd);
        }
        // Auto-advance after triage (UX-12).
        requests
            .into_iter()
            .flat_map(|request| {
                let next = app_front::advance::after(state, &request.cmd).map(Request::user);
                std::iter::once(request).chain(next)
            })
            .collect()
    }

    fn input_inner(&mut self, input: Input, state: &AppState) -> Vec<Request> {
        self.palette.sync(state);
        match input {
            Input::Paste(text) if self.setup.is_some() => {
                if let Some(setup) = &mut self.setup {
                    setup.paste(&text);
                }
                Vec::new()
            }
            Input::Paste(text) if state.view.palette_open => {
                self.palette.paste(&text);
                Vec::new()
            }
            Input::Paste(text) if self.typing(state) => {
                let now = self.now_ms();
                self.composer.paste(&text, now);
                Vec::new()
            }
            Input::Key(key) if key.kind != KeyEventKind::Release => self.key(key, state),
            Input::Mouse(mouse) => self.mouse(mouse, state),
            // A paste outside a text field is dropped, never run as keys.
            Input::Key(_) | Input::Paste(_) | Input::Resize(..) | Input::Tick => Vec::new(),
        }
    }

    fn key(&mut self, key: KeyEvent, state: &AppState) -> Vec<Request> {
        let plain = key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT;
        let ctrl_c =
            key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL);
        if self.confirm_quit {
            self.confirm_quit = false;
            if ctrl_c || (plain && key.code == KeyCode::Char('q')) {
                self.quit = true;
            } else {
                self.notify("Still here; the send goes out when its window closes.");
            }
            return Vec::new();
        }
        if ctrl_c {
            self.request_quit(state);
            return Vec::new();
        }
        if let Some(setup) = &mut self.setup {
            let outcome = setup.key(key, state);
            if outcome.close {
                self.setup = None;
            }
            return outcome.requests;
        }
        if state.accounts.is_empty() && key.code == KeyCode::Enter && !state.view.palette_open {
            self.setup = Some(Setup::default());
            return Vec::new();
        }
        if state.view.palette_open {
            let outcome = self.palette.key(key, state);
            if outcome.wizard {
                self.setup = Some(Setup::default());
            }
            if let Some(account) = outcome.password {
                self.setup = Setup::reenter(state, account);
            }
            return outcome
                .requests
                .into_iter()
                .map(|r| Request {
                    cmd: expand_home(r.cmd),
                    ..r
                })
                .collect();
        }
        if let Some(view) = &mut self.approvals
            && self.result.is_none()
        {
            let outcome = view.key(key, state);
            if outcome.close {
                self.approvals = None;
            }
            if let Some(text) = outcome.notice {
                self.notify(text);
            }
            return outcome.requests;
        }
        if state.view.settings.is_some() && self.result.is_none() {
            let outcome = self.settings.key(key, state);
            if outcome.wizard {
                self.setup = Some(Setup::default());
            }
            if let Some(step) = outcome.ask {
                self.palette.open_at(step);
                return vec![Request::user(Command::OpenPalette {})];
            }
            if !outcome.unhandled {
                return outcome.requests;
            }
        }
        if self.typing(state) {
            return self.compose_key(key, state);
        }
        if let Some(result) = &mut self.result {
            match key.code {
                KeyCode::Down => result.scroll_by(1),
                KeyCode::Up => result.scroll_by(-1),
                KeyCode::Char(' ') | KeyCode::PageDown => result.scroll_by(result.page()),
                KeyCode::Char('b') | KeyCode::PageUp => result.scroll_by(-result.page()),
                KeyCode::Char('r') => result.toggle_raw(),
                KeyCode::Tab => result.select(true),
                KeyCode::BackTab => result.select(false),
                KeyCode::Enter => {
                    if let Some(cmd) = result.chosen() {
                        self.result = None;
                        return vec![Request::user(cmd)];
                    }
                }
                KeyCode::Esc | KeyCode::Char('q' | '?') | KeyCode::Backspace => self.result = None,
                _ => {}
            }
            return Vec::new();
        }
        // TUI-local keys: scrolling, and `q`/Backspace standing in for Escape.
        match key.code {
            KeyCode::Char(' ') | KeyCode::PageDown if plain => {
                let page = self.reader.page();
                self.reader.scroll_by(page);
                return Vec::new();
            }
            KeyCode::Char('b') | KeyCode::PageUp if plain => {
                let page = self.reader.page();
                self.reader.scroll_by(-page);
                return Vec::new();
            }
            KeyCode::Down if plain && self.reader_active(state) => {
                self.reader.scroll_by(1);
                return Vec::new();
            }
            KeyCode::Up if plain && self.reader_active(state) => {
                self.reader.scroll_by(-1);
                return Vec::new();
            }
            KeyCode::Char('q') if plain && !self.keys.is_pending() => {
                return self.back(state, true);
            }
            KeyCode::Char('m') if key.modifiers.is_empty() && !self.keys.is_pending() => {
                self.mouse = !self.mouse;
                self.external = Some(External::MouseCapture(self.mouse));
                self.notify(if self.mouse {
                    "Mouse on: tap to select, swipe to scroll"
                } else {
                    "Mouse off: select text with the terminal; m turns it back on"
                });
                return Vec::new();
            }
            KeyCode::Char('?') if !self.keys.is_pending() => {
                self.result = Some(ResultView::text("Keys", HELP_NOTE, help(state, self.lang)));
                return Vec::new();
            }
            KeyCode::Backspace if key.modifiers.is_empty() && !self.keys.is_pending() => {
                return self.back(state, false);
            }
            _ => {}
        }
        let Some(chord) = keys::chord_of(&key) else {
            return Vec::new();
        };
        let now = self.now_ms();
        match self.keys.press(chord, false, state, now) {
            Press::Done(action) => self.act(state, action),
            Press::Pending | Press::Unbound => Vec::new(),
        }
    }

    /// A tap selects a row (a second tap opens it) or opens a mailbox; the wheel (a swipe)
    /// scrolls the list or the reader. Over the palette and the read-only views only the wheel
    /// acts.
    fn mouse(&mut self, mouse: MouseEvent, state: &AppState) -> Vec<Request> {
        let step: isize = match mouse.kind {
            MouseEventKind::ScrollDown => 3,
            MouseEventKind::ScrollUp => -3,
            MouseEventKind::Down(MouseButton::Left) => 0,
            _ => return Vec::new(),
        };
        if let Some(result) = &mut self.result {
            result.scroll_by(step);
            return Vec::new();
        }
        if state.view.palette_open || self.approvals.is_some() || self.confirm_quit {
            return Vec::new();
        }
        let at = ratatui::layout::Position::new(mouse.column, mouse.row);
        let Some(hit) = self
            .hits
            .iter()
            .rev()
            .find(|(rect, _)| rect.contains(at))
            .map(|(_, hit)| *hit)
        else {
            return Vec::new();
        };
        if step != 0 {
            match hit {
                Hit::Reader => self.reader.scroll_by(step),
                Hit::List | Hit::Thread(_) => {
                    self.list_offset = self.list_offset.saturating_add_signed(step);
                }
                Hit::Mailbox(_) => {}
            }
            return Vec::new();
        }
        let cursor = state.view.selected.last().copied().or(state.view.thread);
        match hit {
            Hit::Thread(id) if cursor == Some(id) && state.view.thread != Some(id) => {
                vec![Request::user(Command::OpenThread { id })]
            }
            Hit::Thread(id) if cursor != Some(id) => {
                vec![Request::user(Command::SelectThreads { ids: vec![id] })]
            }
            Hit::Mailbox(id) => vec![Request::user(Command::OpenMailbox { id })],
            _ => Vec::new(),
        }
    }

    /// Whether keys go to the composer: it is shown, has the focus, and nothing is over it.
    fn typing(&self, state: &AppState) -> bool {
        state.view.focus == Pane::Composer
            && layout::composer_open(state)
            && self.slots.contains(&Slot::Composer)
            && self.result.is_none()
            && self.approvals.is_none()
            && !state.view.palette_open
    }

    /// A key in the composer: the registry's Escape and Ctrl chords win (typed text is written
    /// back first, so that Ctrl+S sends what is on screen); everything else edits.
    fn compose_key(&mut self, key: KeyEvent, state: &AppState) -> Vec<Request> {
        let now = self.now_ms();
        if key.code == KeyCode::Char('o')
            && key.modifiers == KeyModifiers::CONTROL
            && let Some(draft) = state.view.composer.and_then(|id| state.drafts.get(&id))
            && !draft.is_outgoing()
        {
            // The body in `$VISUAL`/`$EDITOR`; what was typed is written back first.
            let out: Vec<Request> = self.composer.flush(state).into_iter().collect();
            self.external = Some(External::EditBody {
                draft: draft.id,
                text: self.composer.body_text(),
            });
            return out;
        }
        if let Some(chord) = keys::chord_of(&key)
            && let Press::Done(action) = self.keys.press(chord, true, state, now)
        {
            let mut out: Vec<Request> = self.composer.flush(state).into_iter().collect();
            out.extend(self.act(state, action));
            return out;
        }
        self.composer.key(key, state, now)
    }

    /// Escape, as `q` or Backspace: closes the innermost thing that is open; with the mailboxes
    /// focused it goes back to the list; on the top level `q` quits.
    fn back(&mut self, state: &AppState, quits: bool) -> Vec<Request> {
        let escape = Chord::plain(Key::Escape);
        let now = self.now_ms();
        if let Press::Done(action) = self.keys.press(escape, false, state, now) {
            return self.act(state, action);
        }
        if state.view.focus == Pane::Sidebar && state.view.can_focus(Pane::List) {
            return vec![Request::user(Command::FocusPane { pane: Pane::List })];
        }
        if quits {
            self.request_quit(state);
        }
        Vec::new()
    }

    /// Quits, unless this TUI owns the session and a send waits in its undo window: closing
    /// now would cancel it, so that is asked first.
    fn request_quit(&mut self, state: &AppState) {
        let scheduled = state
            .drafts
            .values()
            .filter(|d| d.send_at.is_some() && !d.sending)
            .count();
        if self.owns_session && scheduled > 0 {
            self.confirm_quit = true;
        } else {
            self.quit = true;
        }
    }

    fn act(&mut self, state: &AppState, action: Action) -> Vec<Request> {
        match action {
            Action::Run(cmd) => vec![Request::user(expand_home(cmd))],
            Action::Ask(step) => {
                // A command that still needs an argument opens the palette at that step.
                self.palette.open_at(step);
                vec![Request::user(Command::OpenPalette {})]
            }
            Action::Wizard => {
                self.setup = Some(Setup::default());
                Vec::new()
            }
            Action::Password(account) => {
                self.setup = Setup::reenter(state, account);
                Vec::new()
            }
        }
    }

    /// Takes the results of earlier requests; returns follow-up requests.
    pub fn replies(&mut self, replies: Vec<Reply>, state: &AppState) -> Vec<Request> {
        let mut out = Vec::new();
        for reply in replies {
            match reply.tag {
                Purpose::Bodies(thread) => {
                    let receipt = reply.result.as_ref().ok().and_then(|r| r.as_ref().ok());
                    self.bodies.accept(thread, receipt);
                }
                Purpose::Setup => {
                    if let Some(setup) = &mut self.setup
                        && let Some(request) = setup.accept(&reply)
                    {
                        out.push(request);
                    }
                }
                Purpose::User => {
                    match &reply.result {
                        Ok(Ok(receipt)) => {
                            if let Some(draft) = self.last.observe(receipt) {
                                self.sending = Some(draft);
                            }
                        }
                        Ok(Err(err)) => {
                            let title = reply.name.spec().title;
                            self.notify(notice::rejection(title, err));
                        }
                        Err(err) => self.notify(format!("dail: {err}")),
                    }
                    if reply.name == CommandName::ListApprovals && reply.result.is_ok() {
                        // The requests view rather than their JSON.
                        self.approvals = Some(ApprovalsView::default());
                    } else if let Ok(Ok(receipt)) = &reply.result
                        && reply.name.kind() == app_core::CommandKind::Query
                    {
                        self.result = Some(ResultView::new(
                            reply.name,
                            &receipt.data,
                            state,
                            self.now_ms(),
                        ));
                    }
                    // A new draft (compose, reply, forward) opens in the composer, as in the GUI.
                    if let Ok(Ok(receipt)) = &reply.result
                        && creates_draft(reply.name)
                        && let Ok(id) = serde_json::from_value(receipt.data["id"].clone())
                    {
                        out.push(Request::user(Command::OpenComposer { id }));
                    }
                }
            }
        }
        out
    }
}

const HELP_NOTE: &str = "Problems, the keys of the registry, the TUI's own. q, Esc or ? closes.";

/// The help text: the full text of current problems, every registry binding that a terminal
/// delivers (with the command's title), and the TUI-local keys.
fn help(state: &AppState, lang: app_front::i18n::Lang) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let problems = app_front::status::problems(state, lang);
    if !problems.is_empty() {
        out.push_str("Problems\n");
        for problem in problems {
            let _ = writeln!(out, "{}: {}", problem.text, problem.detail);
            if let Some(hint) = problem.hint {
                let _ = writeln!(out, "  {hint}");
            }
        }
        out.push('\n');
    }
    for &name in CommandName::ALL {
        let spec = name.spec();
        let shown = keys::deliverable(&state.settings, name);
        if !shown.is_empty() {
            let _ = writeln!(out, "{:<14} {}", shown.join("  "), spec.title);
        }
    }
    out.push_str(
        "\nIn the TUI\n\
         Space b        page the reader down / up (also PageDown, PageUp)\n\
         ↑ ↓            scroll a lone or focused reader\n\
         q Backspace    back (Escape); q on the top level quits\n\
         ?              this help\n\
         m              mouse on/off (off: the terminal selects and copies text)\n\
         Ctrl+C         quit\n\
         Ctrl+K :       the palette: every command, also those without a key\n",
    );
    out
}

/// `~/` at the start of an attachment's path is the user's home directory (there is no file
/// dialog over SSH, so paths are typed).
fn expand_home(cmd: Command) -> Command {
    match cmd {
        Command::AttachFile { draft, path, index } => {
            let path = match (path.strip_prefix("~/"), home_dir()) {
                (Some(rest), Some(home)) => home.join(rest).to_string_lossy().into_owned(),
                _ => path,
            };
            Command::AttachFile { draft, path, index }
        }
        cmd => cmd,
    }
}

fn home_dir() -> Option<std::path::PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var)
        .filter(|v| !v.is_empty())
        .map(std::path::PathBuf::from)
}
