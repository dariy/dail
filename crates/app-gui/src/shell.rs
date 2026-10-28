//! `Shell`: one frame of the UI, as a pure function of `AppState` plus UI-local state.
//!
//! The shell reads `&AppState` and never changes it. Everything the user does becomes a
//! [`Request`] pushed into the frame-local queue that [`Shell::frame`] returns; the app hands
//! the queue to the session after the frame. Tests drive the shell headlessly with
//! `egui::Context::run_ui`.

use app_core::{AppState, Command, CommandName};
use egui::Panel;

use crate::approvals;
use app_front::body::Bodies;

use crate::attachments::{FolderPicker, PartKey, PreviewData, Previews, Saver};
use crate::bridge::{Background, Purpose, Reply, Request};
use crate::composer::ComposerState;
use crate::images::{ImageKey, Images};
use crate::keymap::{self, KeyState};
use crate::list::{self, ListState};
use crate::palette::PaletteState;
use crate::reader;
use crate::sidebar;
use crate::theme;
use crate::toasts::{ToastKind, Toasts};
use crate::topbar;
use crate::ui;
use crate::wizard::Wizard;

/// Something the user asked for that happens outside the engine: the app performs it after
/// the frame (`ARCHITECTURE.md §7.1`, reader).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum External {
    /// Open a link from a message in the system browser (only `http`, `https` and `mailto`).
    OpenUrl(String),
    /// Open the sanitized original of a stored message body in the system browser.
    OpenOriginal {
        /// The body's blob ID (`BodyState::Fetched`).
        blob: String,
    },
    /// Read and decode an inline image of a stored message body (`Images::accept` takes it).
    LoadImage(ImageKey),
    /// Read an attachment for its preview (`Shell::accept_preview` takes it).
    LoadPreview(PartKey),
    /// Open a private temporary copy of an attachment (a PDF) in the browser's viewer.
    OpenAttachment(PartKey),
}

/// UI-local state that is not part of `AppState` (form inputs, caches, toasts).
pub struct Shell {
    externals: Vec<External>,
    notices: Vec<String>,
    list: ListState,
    rows: list::RowCache,
    bodies: Bodies,
    images: Images,
    folds: reader::Folds,
    saver: Saver,
    previews: Previews,
    palette: PaletteState,
    keys: KeyState,
    composer: ComposerState,
    /// The reply docked under the conversation, to scroll the reader once when it opens.
    docked: Option<app_core::DraftId>,
    toasts: Toasts,
    wizard: Wizard,
    settings: crate::settings::SettingsState,
    /// The last query result the user asked for, shown as `app_front::result`'s view or raw.
    result: Option<QueryResult>,
    clock: fn() -> i64,
    /// The mark-as-read timer (UX-5).
    autoread: app_front::autoread::AutoRead,
    /// The language of the shared texts (`app_front::i18n`).
    lang: app_front::i18n::Lang,
}

impl Default for Shell {
    fn default() -> Self {
        Self {
            externals: Vec::new(),
            notices: Vec::new(),
            list: ListState::default(),
            rows: list::RowCache::default(),
            bodies: Bodies::rich(),
            images: Images::default(),
            folds: reader::Folds::default(),
            saver: Saver::default(),
            previews: Previews::default(),
            palette: PaletteState::default(),
            keys: KeyState::default(),
            composer: ComposerState::default(),
            docked: None,
            toasts: Toasts::default(),
            wizard: Wizard::default(),
            settings: crate::settings::SettingsState::default(),
            result: None,
            clock: system_now_ms,
            autoread: app_front::autoread::AutoRead::default(),
            lang: app_front::i18n::Lang::default(),
        }
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

impl Shell {
    /// Shows the shared texts in `lang`.
    pub fn set_lang(&mut self, lang: app_front::i18n::Lang) {
        self.lang = lang;
        self.composer.lang = lang;
    }

    /// A shell that reads the time from `clock` (Unix ms) instead of the system clock.
    pub fn with_clock(clock: fn() -> i64) -> Self {
        Self {
            clock,
            ..Self::default()
        }
    }

    /// What the user asked for outside the engine since the last call.
    pub fn take_external(&mut self) -> Vec<External> {
        std::mem::take(&mut self.externals)
    }

    /// A shell whose *Save…* buttons ask `picker` for the folder instead of the native dialog.
    #[must_use]
    pub fn with_folder_picker(self, picker: FolderPicker) -> Self {
        Self {
            saver: Saver::with_picker(picker),
            ..self
        }
    }

    /// Takes a loaded inline image (`External::LoadImage`) for the reader.
    pub fn accept_image(
        &mut self,
        ctx: &egui::Context,
        key: ImageKey,
        image: Result<egui::ColorImage, String>,
    ) {
        self.images.accept(ctx, key, image);
    }

    /// Takes a loaded attachment preview (`External::LoadPreview`) for the reader.
    pub fn accept_preview(
        &mut self,
        ctx: &egui::Context,
        key: PartKey,
        data: Result<PreviewData, String>,
    ) {
        self.previews.accept(ctx, key, data);
    }

    /// Shows a failure of background work (e.g. opening the original) as a toast.
    pub fn notify(&mut self, text: String) {
        self.notices.push(text);
    }

    /// Renders one frame and returns the commands the user issued in it, in order.
    pub fn frame(
        &mut self,
        ui: &mut egui::Ui,
        state: &AppState,
        replies: Vec<Reply>,
    ) -> Vec<Request> {
        let queue = self.frame_body(ui, state, replies);
        // Auto-advance after triage (UX-12): the neighbour opens right after.
        let mut queue: Vec<Request> = queue
            .into_iter()
            .flat_map(|request| {
                let next = (request.purpose == Purpose::User)
                    .then(|| app_front::advance::after(state, &request.cmd))
                    .flatten()
                    .map(Request::user);
                std::iter::once(request).chain(next)
            })
            .collect();
        // Mark as read after a delay (UX-5): the user's own, undoable `mark_read`.
        for request in &queue {
            if request.purpose == Purpose::User {
                self.autoread.note(&request.cmd);
            }
        }
        let focused = ui.ctx().input(|i| i.viewport().focused) != Some(false);
        match self.autoread.poll(state, (self.clock)(), focused) {
            app_front::autoread::Poll::Fire(cmd) => queue.push(Request::user(cmd)),
            app_front::autoread::Poll::Wait(ms) => ui
                .ctx()
                .request_repaint_after(std::time::Duration::from_millis(ms)),
            app_front::autoread::Poll::Idle => {}
        }
        queue
    }

    fn frame_body(
        &mut self,
        ui: &mut egui::Ui,
        state: &AppState,
        replies: Vec<Reply>,
    ) -> Vec<Request> {
        let mut queue: Vec<Request> = Vec::new();
        let now = ui.input(|i| i.time);
        for reply in &replies {
            self.on_reply(reply, now, &mut queue);
        }
        for notice in self.notices.drain(..) {
            self.toasts.push(notice, ToastKind::Error, now);
        }
        if let Some(notice) = theme::apply(ui.ctx(), &state.settings) {
            self.toasts.push(notice, ToastKind::Error, now);
        }
        theme::zoom(ui.ctx(), state.settings.text_scale);
        let t = theme::tokens(ui.ctx());
        let now_ms = (self.clock)();
        self.saver.receive(&mut queue);
        if let Some((thread, cmd)) = self.bodies.refresh(state) {
            queue.push(Request {
                purpose: Purpose::Background(Background::ReaderBodies(thread)),
                cmd,
            });
        }
        // A copy (~1.6 MB at 100k) is cheaper than borrowing the cache across the frame.
        let rows = self.rows.of(state).clone();
        let ctx = ui.ctx().clone();
        let asked = self.palette.show(&ctx, &t, state, &mut queue);
        if asked.wizard {
            self.wizard.open();
        }
        if let Some(account) = asked.password {
            self.wizard.reenter(state, account);
        }
        self.wizard.show(&ctx, &t, state, &mut queue);
        if !state.view.palette_open && !self.wizard.is_open() {
            let actions = self.keys.handle(&ctx, state);
            if !actions.is_empty() {
                // Typed text goes first, so that e.g. Ctrl+Enter sends what is on screen.
                self.composer.flush(state, &mut queue);
            }
            for action in actions {
                match action {
                    keymap::Action::Run(cmd) => queue.push(Request::user(cmd)),
                    keymap::Action::Ask(step) => {
                        self.palette.open_at(step);
                        queue.push(Request::user(Command::OpenPalette {}));
                    }
                    keymap::Action::Wizard => self.wizard.open(),
                    keymap::Action::Password(account) => self.wizard.reenter(state, account),
                }
            }
        }
        self.show_result(&ctx, &t, state, now_ms, &mut queue);
        self.toasts.show(&ctx, &t, state, now_ms, &mut queue);

        // The frame (UI-5): the sidebar at full height, the top bar over the rest, then the panes.
        // The sidebar and the top bar draw with the chrome group (dark in `dail-light`).
        let chrome = t.chrome();
        let bar = egui::Frame::new()
            .fill(chrome.base)
            .inner_margin(egui::Margin::symmetric(t.m.space.md as i8, 0));
        let sidebar = if state.view.sidebar_collapsed {
            Panel::left("sidebar-rail")
                .exact_size(t.m.frame.rail)
                .resizable(false)
                .frame(egui::Frame::new().fill(chrome.base))
                .show(ui, |ui| {
                    ui.style_mut().visuals = theme::chrome_visuals(&t);
                    sidebar::rail(ui, &chrome, state, &mut queue)
                })
                .inner
        } else {
            Panel::left("sidebar")
                .default_size(t.m.frame.sidebar)
                .size_range(t.m.frame.sidebar_min..=t.m.frame.sidebar_max)
                .resizable(true)
                .frame(bar)
                .show(ui, |ui| {
                    ui.style_mut().visuals = theme::chrome_visuals(&t);
                    sidebar::show(ui, &chrome, state, &mut queue)
                })
                .inner
        };
        if sidebar.add_account {
            self.wizard.open();
        }
        if sidebar.settings {
            queue.push(Request::user(Command::OpenSettings { section: None }));
        }
        if let Some(menu) = sidebar.folder {
            match menu.flow(state) {
                app_front::palette::Flow::Run(cmd) => queue.push(Request::user(cmd)),
                app_front::palette::Flow::Ask(step) => {
                    self.palette.open_at(step);
                    queue.push(Request::user(Command::OpenPalette {}));
                }
                _ => {}
            }
        }
        let top = Panel::top("top-bar")
            .exact_size(t.m.frame.top_bar)
            .frame(bar.stroke(egui::Stroke::new(t.m.stroke.hairline, chrome.border_subtle)))
            .show(ui, |ui| {
                ui.style_mut().visuals = theme::chrome_visuals(&t);
                topbar::show(ui, &chrome, state, now_ms, self.lang, &mut queue)
            });
        if let Some(account) = top.inner {
            self.wizard.reenter(state, account);
        }
        if let Some(section) = state.view.settings {
            // The settings page takes the place of the list and the reader (UI-12).
            let page = egui::CentralPanel::default()
                .frame(
                    egui::Frame::new()
                        .fill(t.canvas)
                        .inner_margin(egui::Margin::same(t.m.frame.reader_padding as i8)),
                )
                .show(ui, |ui| {
                    self.settings
                        .show(ui, &t, state, section, now_ms, &mut queue)
                })
                .inner;
            if page.wizard {
                self.wizard.open();
            }
            if let Some(account) = page.password {
                self.wizard.reenter(state, account);
            }
            if let Some(step) = page.ask {
                self.palette.open_at(step);
                queue.push(Request::user(Command::OpenPalette {}));
            }
            if let Some(url) = page.open {
                self.externals.push(External::OpenUrl(url));
            }
            return queue;
        }
        Panel::left("thread-list")
            .default_size(t.m.frame.list)
            .size_range(t.m.frame.list_min..=t.m.frame.list_max)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(t.base)
                    .inner_margin(egui::Margin::ZERO),
            )
            .show(ui, |ui| {
                let undo = self.toasts.undo_label().map(str::to_owned);
                Panel::bottom("list-footer")
                    .exact_size(t.m.frame.pane_footer)
                    .frame(
                        egui::Frame::new()
                            .inner_margin(egui::Margin::symmetric(t.m.space.md as i8, 0))
                            .stroke(egui::Stroke::new(t.m.stroke.hairline, t.border_subtle)),
                    )
                    .show_separator_line(false)
                    .show(ui, |ui| {
                        list::footer(ui, &t, state, &rows, undo.as_deref(), &mut queue);
                    });
                egui::Frame::new()
                    .inner_margin(egui::Margin::ZERO)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        list::show(ui, &t, state, &rows, &mut self.list, now_ms, &mut queue);
                        if let Some(step) = self.list.take_ask() {
                            self.palette.open_at(step);
                            queue.push(Request::user(Command::OpenPalette {}));
                        }
                    });
            });
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(t.canvas)
                    .inner_margin(egui::Margin::symmetric(
                        t.m.frame.reader_padding as i8,
                        t.m.space.sm as i8,
                    )),
            )
            .show(ui, |ui| {
                if app_front::updates::ask_first_run(state) {
                    ui::callout(&t).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        updates_question(ui, &t, &mut queue);
                    });
                    ui.add_space(t.m.space.sm);
                }
                if !state.approvals.is_empty() {
                    ui::callout(&t).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        approvals::show(ui, &t, state, &mut queue);
                    });
                    ui.add_space(t.m.space.sm);
                }
                match placement(state) {
                    Placement::Pane => self.composer.show(ui, &t, state, now_ms, &mut queue),
                    Placement::None => {
                        self.docked = None;
                        self.show_reader(ui, &t, state, now_ms, &rows.ids, false, &mut queue);
                    }
                    Placement::Docked(draft) => {
                        // A reply to the open conversation shows under it (UI-13). The reader
                        // scrolls to its end once, when the composer opens.
                        let opened = self.docked != Some(draft);
                        self.docked = Some(draft);
                        Panel::bottom("reply-composer")
                            .resizable(true)
                            .default_size(ui.available_height() * 0.55)
                            .min_size(t.m.overlays.palette_list_max / 2.0)
                            .frame(egui::Frame::new().fill(t.canvas))
                            .show(ui, |ui| {
                                self.composer.show(ui, &t, state, now_ms, &mut queue);
                            });
                        self.show_reader(ui, &t, state, now_ms, &rows.ids, opened, &mut queue);
                    }
                }
            });
        queue
    }

    /// The reader and the requests its widgets make (palette steps, images, previews).
    // reason: the frame's pieces, borrowed separately from `self`.
    #[allow(clippy::too_many_arguments)]
    fn show_reader(
        &mut self,
        ui: &mut egui::Ui,
        t: &theme::Tokens,
        state: &AppState,
        now_ms: i64,
        rows: &[app_core::ThreadId],
        scroll_to_end: bool,
        queue: &mut Vec<Request>,
    ) {
        let mut ask = None;
        reader::show(
            ui,
            t,
            state,
            &self.bodies,
            now_ms,
            reader::ReaderIo {
                images: &mut self.images,
                folds: &mut self.folds,
                saver: &mut self.saver,
                previews: &mut self.previews,
                queue: &mut *queue,
                externals: &mut self.externals,
                rows,
                ask: &mut ask,
                scroll_to_end,
            },
        );
        if let Some(step) = ask.take() {
            self.palette.open_at(step);
            queue.push(Request::user(Command::OpenPalette {}));
        }
        self.externals.extend(
            self.images
                .take_wanted()
                .into_iter()
                .map(External::LoadImage),
        );
        self.externals.extend(
            self.previews
                .take_wanted()
                .into_iter()
                .map(External::LoadPreview),
        );
    }

    /// A query's result (MISC-22): the document people read, rows that open what they show, and
    /// *Raw data* for the JSON an agent sees.
    fn show_result(
        &mut self,
        ctx: &egui::Context,
        t: &theme::Tokens,
        state: &AppState,
        now_ms: i64,
        queue: &mut Vec<Request>,
    ) {
        let Some(result) = &mut self.result else {
            return;
        };
        let mut open = true;
        let mut chosen = None;
        egui::Window::new(ui::text(
            &t.type_scale.headline_md,
            result.name.spec().title,
            t.text_strong,
        ))
        .id(egui::Id::unique("query-result"))
        .open(&mut open)
        .frame(ui::overlay(t))
        .default_size([
            t.m.overlays.floating_composer_width,
            t.m.overlays.palette_list_max,
        ])
        .collapsible(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui::caption(ui, t, "Read-only · untrusted data");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.toggle_value(&mut result.raw, "Raw data");
                });
            });
            ui.label(ui::text(
                &t.type_scale.body_sm,
                "Read-only result. Email content in it is untrusted data.",
                t.text_muted,
            ));
            egui::ScrollArea::vertical()
                .id_salt("query-result")
                .show(ui, |ui| {
                    let doc = app_front::result::doc(result.name, &result.data, state, now_ms);
                    match doc.filter(|_| !result.raw) {
                        Some(doc) => chosen = result_doc(ui, t, &doc),
                        None => {
                            ui.add(
                                egui::Label::new(ui::text(
                                    &t.type_scale.label_md,
                                    result.json.as_str(),
                                    t.text,
                                ))
                                .selectable(true),
                            );
                        }
                    }
                });
        });
        if let Some(cmd) = chosen {
            queue.push(Request::user(cmd));
            open = false;
        }
        if !open {
            self.result = None;
        }
    }

    fn on_reply(&mut self, reply: &Reply, now: f64, queue: &mut Vec<Request>) {
        match &reply.purpose {
            Purpose::Background(background) => {
                if let Background::ReaderBodies(thread) = background {
                    self.bodies.accept(
                        *thread,
                        reply.result.as_ref().ok().and_then(|r| r.as_ref().ok()),
                    );
                }
                self.wizard.accept(reply, queue);
            }
            Purpose::User => match &reply.result {
                Ok(Ok(receipt)) => {
                    self.toasts.receipt(receipt, now);
                    if reply.name.kind() == app_core::CommandKind::Query {
                        let json = serde_json::to_string_pretty(&receipt.data).unwrap_or_default();
                        self.result = Some(QueryResult {
                            name: reply.name,
                            data: receipt.data.clone(),
                            json: app_front::text::clean_lines(&json).join("\n"),
                            raw: false,
                        });
                    }
                    if creates_draft(reply.name)
                        && let Ok(id) = serde_json::from_value(receipt.data["id"].clone())
                    {
                        queue.push(Request::user(Command::OpenComposer { id }));
                    }
                }
                Ok(Err(err)) => {
                    tracing::debug!(command = reply.name.as_str(), error = %err, "command rejected");
                    self.toasts.error(reply.name.spec().title, err, now);
                }
                Err(err) => {
                    tracing::warn!(command = reply.name.as_str(), error = %err, "session failed");
                    self.toasts
                        .push(format!("dail: {err}"), ToastKind::Error, now);
                }
            },
        }
    }
}

/// Where the composer shows (UI-13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Placement {
    /// No draft is open: the reader.
    None,
    /// New drafts and forwards take the whole pane.
    Pane,
    /// A reply to a message of the open conversation shows under it.
    Docked(app_core::DraftId),
}

/// The first-run question about update checks (Wave 7 F3): it stays until it is answered.
fn updates_question(ui: &mut egui::Ui, t: &theme::Tokens, queue: &mut Vec<Request>) {
    use app_front::updates::{QUESTION, QUESTION_DETAIL, answer};

    ui.label(ui::text(&t.type_scale.headline_md, QUESTION, t.text_strong));
    ui.label(ui::text(
        &t.type_scale.body_sm,
        QUESTION_DETAIL,
        t.text_secondary,
    ));
    ui.add_space(t.m.space.xs);
    ui.horizontal(|ui| {
        if ui::primary(ui, t, "Check for updates", None, true) {
            queue.push(Request::user(answer(true)));
        }
        if ui::secondary(ui, t, "No thanks", None, true) {
            queue.push(Request::user(answer(false)));
        }
    });
}

fn placement(state: &AppState) -> Placement {
    let Some(draft) = state.view.composer.and_then(|id| state.drafts.get(&id)) else {
        return Placement::None;
    };
    let reply = draft
        .quote
        .as_ref()
        .is_some_and(|q| q.style == app_core::QuoteStyle::Reply);
    let in_thread = draft.reply_to.is_some_and(|message| {
        state
            .view
            .thread
            .and_then(|id| state.threads.get(&id))
            .is_some_and(|thread| thread.messages.contains(&message))
    });
    if reply && in_thread {
        Placement::Docked(draft.id)
    } else {
        Placement::Pane
    }
}

/// A query's result, until it is closed.
struct QueryResult {
    name: CommandName,
    data: serde_json::Value,
    /// The pretty JSON, cleaned, for *Raw data*.
    json: String,
    raw: bool,
}

/// Draws a result document; returns the command of a clicked row.
fn result_doc(
    ui: &mut egui::Ui,
    t: &theme::Tokens,
    doc: &app_front::result::ResultDoc,
) -> Option<Command> {
    let mut chosen = None;
    for section in &doc.sections {
        if let Some(title) = &section.title {
            ui.add_space(t.m.space.sm);
            ui::caption(ui, t, title);
        }
        egui::Grid::new(ui.next_auto_id())
            .num_columns(2)
            .spacing([t.m.space.md, t.m.space.xs])
            .show(ui, |ui| {
                for field in &section.fields {
                    ui.label(ui::text(&t.type_scale.label_md, &field.label, t.text_muted));
                    ui.add(
                        egui::Label::new(ui::text(&t.type_scale.body_md, &field.value, t.text))
                            .selectable(true),
                    );
                    ui.end_row();
                }
            });
        for row in &section.rows {
            use app_front::result::Mark;
            let mut line = row.title.clone();
            if let Some(meta) = &row.meta {
                line.push_str(&format!("  ·  {meta}"));
            }
            let unread = row.marks.contains(&Mark::Unread);
            let response = ui
                .horizontal(|ui| {
                    for mark in &row.marks {
                        let icon = match mark {
                            // Unread is the bold title, as in the list.
                            Mark::Unread => continue,
                            Mark::Starred => app_theme::Icon::Star,
                            Mark::Pinned => app_theme::Icon::PushPin,
                            Mark::Attachment => app_theme::Icon::AttachFile,
                            Mark::Passed => app_theme::Icon::Check,
                            Mark::Failed => app_theme::Icon::Close,
                            Mark::Pending => app_theme::Icon::Schedule,
                        };
                        ui::icon(ui, icon, t.m.controls.icon_xs, t.text_strong, true);
                    }
                    let mut text = ui::text(&t.type_scale.body_md, &line, t.text_strong);
                    if unread {
                        text = text.strong();
                    }
                    ui.add(egui::Label::new(text).sense(if row.command.is_some() {
                        egui::Sense::click()
                    } else {
                        egui::Sense::hover()
                    }))
                })
                .inner;
            if let Some(detail) = &row.detail {
                ui.label(ui::text(&t.type_scale.body_sm, detail, t.text_muted));
            }
            if response.clicked() {
                chosen.clone_from(&row.command);
            }
            ui.separator();
        }
    }
    chosen
}
