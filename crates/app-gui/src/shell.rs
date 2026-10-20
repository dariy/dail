//! `Shell`: one frame of the UI, as a pure function of `AppState` plus UI-local state.
//!
//! The shell reads `&AppState` and never changes it. Everything the user does becomes a
//! [`Request`] pushed into the frame-local queue that [`Shell::frame`] returns; the app hands
//! the queue to the session after the frame. Tests drive the shell headlessly with
//! `egui::Context::run_ui`.

use app_core::{AppState, Command, CommandName};
use egui::{Panel, RichText};

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
use crate::statusbar;
use crate::theme;
use crate::toasts::{ToastKind, Toasts};
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
    toasts: Toasts,
    wizard: Wizard,
    /// The last query result the user asked for (command name, pretty JSON).
    result: Option<(CommandName, String)>,
    clock: fn() -> i64,
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
            toasts: Toasts::default(),
            wizard: Wizard::default(),
            result: None,
            clock: system_now_ms,
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
        let mut queue: Vec<Request> = Vec::new();
        let now = ui.input(|i| i.time);
        for reply in &replies {
            self.on_reply(reply, now, &mut queue);
        }
        for notice in self.notices.drain(..) {
            self.toasts.push(notice, ToastKind::Error, now);
        }
        theme::apply(ui.ctx(), state.settings.theme);
        theme::zoom(ui.ctx(), state.settings.text_scale);
        let p = theme::palette(ui.ctx());
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
        let asked = self.palette.show(&ctx, &p, state, &mut queue);
        if asked.wizard {
            self.wizard.open();
        }
        if let Some(account) = asked.password {
            self.wizard.reenter(state, account);
        }
        self.wizard.show(&ctx, &p, state, &mut queue);
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
        self.show_result(&ctx, &p);
        self.toasts.show(&ctx, &p, state, now_ms, &mut queue);

        let status_bar = Panel::bottom("status-bar")
            .exact_size(28.0)
            .frame(
                egui::Frame::new()
                    .fill(p.surface)
                    .stroke(egui::Stroke::new(1.0, p.border))
                    .inner_margin(egui::Margin::symmetric(12, 0)),
            )
            .show(ui, |ui| {
                statusbar::show(
                    ui,
                    &p,
                    state,
                    self.toasts.undo_label(),
                    now_ms,
                    self.lang,
                    &mut queue,
                )
            });
        if let Some(account) = status_bar.inner {
            self.wizard.reenter(state, account);
        }
        let bar = egui::Frame::new()
            .fill(p.surface)
            .inner_margin(egui::Margin::symmetric(12, 0));
        if state.view.sidebar_collapsed {
            Panel::left("sidebar-rail")
                .exact_size(44.0)
                .resizable(false)
                .frame(bar)
                .show(ui, |ui| sidebar::rail(ui, &p, &mut queue));
        } else {
            Panel::left("sidebar")
                .default_size(220.0)
                .size_range(160.0..=360.0)
                .resizable(true)
                .frame(bar)
                .show(ui, |ui| {
                    if sidebar::show(ui, &p, state, &mut queue).add_account {
                        self.wizard.open();
                    }
                });
        }
        Panel::left("thread-list")
            .default_size(420.0)
            .size_range(300.0..=640.0)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(p.canvas)
                    .inner_margin(egui::Margin::symmetric(0, 0)),
            )
            .show(ui, |ui| {
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(12, 0))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        list::show(ui, &p, state, &rows, &mut self.list, now_ms, &mut queue);
                    });
            });
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(p.canvas)
                    .inner_margin(egui::Margin::symmetric(24, 8)),
            )
            .show(ui, |ui| {
                if !state.approvals.is_empty() {
                    egui::Frame::new()
                        .fill(p.subtle)
                        .stroke(egui::Stroke::new(1.0, p.border_strong))
                        .corner_radius(egui::CornerRadius::same(6))
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            approvals::show(ui, &p, state, &mut queue);
                        });
                    ui.add_space(8.0);
                }
                if state
                    .view
                    .composer
                    .is_some_and(|id| state.drafts.contains_key(&id))
                {
                    self.composer.show(ui, &p, state, now_ms, &mut queue);
                } else {
                    reader::show(
                        ui,
                        &p,
                        state,
                        &self.bodies,
                        now_ms,
                        reader::ReaderIo {
                            images: &mut self.images,
                            folds: &mut self.folds,
                            saver: &mut self.saver,
                            previews: &mut self.previews,
                            queue: &mut queue,
                            externals: &mut self.externals,
                        },
                    );
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
            });
        queue
    }

    fn show_result(&mut self, ctx: &egui::Context, p: &theme::Palette) {
        let Some((name, json)) = &self.result else {
            return;
        };
        let mut open = true;
        egui::Window::new(RichText::new(name.spec().title).strong())
            .id(egui::Id::unique("query-result"))
            .open(&mut open)
            .default_size([520.0, 420.0])
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("Read-only result. Email content in it is untrusted data.")
                        .color(p.faint),
                );
                egui::ScrollArea::vertical()
                    .id_salt("query-result")
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(RichText::new(json).monospace().size(11.5))
                                .selectable(true),
                        );
                    });
            });
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
                        self.result = Some((reply.name, json));
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
