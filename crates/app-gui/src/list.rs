//! The thread list: the open mailbox or the search results, virtualized (only visible rows are
//! laid out).

use app_core::{AppState, Command, Pane, ThreadId, ThreadSummary, listing};
use app_theme::Icon;
use egui::{Align, Layout, Sense, Ui, Vec2};

use crate::bridge::Request;
use crate::theme::Tokens;
use crate::ui;
use app_front::format;
use app_front::mailbox::mailbox_name;

/// How the rows are laid out: two lines (48), one (40) or three with the snippet (72). The
/// `list_density` setting picks it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Density {
    /// Two lines: the sender and the marks, then the subject.
    #[default]
    Cozy,
    /// One line: sender, subject, date.
    Compact,
    /// Three lines: the sender and the marks, the subject, the snippet (UI-14).
    Preview,
}

impl Density {
    /// The density of `state`'s settings (`list_density`).
    pub fn of(state: &AppState) -> Self {
        match state.settings.list_density {
            1 => Self::Compact,
            2 => Self::Preview,
            _ => Self::Cozy,
        }
    }

    /// The height of a row.
    pub fn height(self, t: &Tokens) -> f32 {
        match self {
            Self::Cozy => t.m.rows.cozy,
            Self::Compact => t.m.rows.compact,
            Self::Preview => t.m.rows.preview,
        }
    }
}

/// UI-local state of the list.
#[derive(Debug, Default)]
pub struct ListState {
    /// The thread the list last scrolled to, so it scrolls only when the selection moves.
    scrolled_to: Option<ThreadId>,
}

/// Renders the list pane.
pub fn show(
    ui: &mut Ui,
    t: &Tokens,
    state: &AppState,
    rows: &Rows,
    local: &mut ListState,
    now_ms: i64,
    queue: &mut Vec<Request>,
) {
    header(ui, t, state, rows.unread, queue);
    let rows = rows.ids.as_slice();
    ui::divider(ui, t);
    if rows.is_empty() {
        let text = if state.view.search.is_some() {
            "No matching conversations."
        } else if state.view.mailbox.is_some() {
            "No conversations here."
        } else {
            "Pick a mailbox."
        };
        ui::empty_state(ui, t, text, &[]);
        return;
    }
    let density = Density::of(state);
    let row_height = density.height(t);
    ui.spacing_mut().item_spacing.y = 0.0;
    let cursor = state.view.selected.last().copied().or(state.view.thread);
    let focused = state.view.focus == Pane::List;
    let mut scroll = egui::ScrollArea::vertical()
        .id_salt("list")
        .auto_shrink([false, false]);
    if cursor != local.scrolled_to
        && let Some(index) = cursor.and_then(|c| rows.iter().position(|r| *r == c))
    {
        let top = index as f32 * row_height;
        let visible = ui.available_height();
        let offset = ui
            .ctx()
            .data(|d| d.get_temp::<f32>(ui.scope_id().with("list-offset")))
            .unwrap_or(0.0);
        if top < offset || top + row_height > offset + visible {
            scroll = scroll.vertical_scroll_offset((top - visible / 3.0).max(0.0));
        }
        local.scrolled_to = cursor;
    }
    let marks = state
        .view
        .search
        .as_deref()
        .map(app_front::highlight::words)
        .unwrap_or_default();
    let output = scroll.show_rows(ui, row_height, rows.len(), |ui, range| {
        for id in &rows[range] {
            if let Some(thread) = state.threads.get(id) {
                let summary = ThreadSummary::of(state, thread);
                let selected = state.view.selected.contains(id);
                let open = state.view.thread == Some(*id);
                let checked = state.view.checked.contains(id);
                let look = RowLook {
                    selected,
                    open,
                    checked,
                    // With several accounts, rows say whose they are.
                    tag: (state.accounts.len() > 1).then(|| t.tag(state, thread.account)),
                    cursor: focused && cursor == Some(*id),
                    marks: &marks,
                    snippet: thread
                        .messages
                        .iter()
                        .rev()
                        .filter_map(|m| state.messages.get(m))
                        .find_map(|m| m.snippet.as_deref()),
                };
                match row(ui, t, &summary, &look, density, now_ms) {
                    Some(Click::Open) => {
                        queue.push(Request::user(Command::OpenThread { id: *id }));
                    }
                    Some(Click::Toggle) => queue.push(Request::user(if checked {
                        Command::UncheckThreads { ids: vec![*id] }
                    } else {
                        Command::CheckThreads { ids: vec![*id] }
                    })),
                    Some(Click::Range) => {
                        let ids = span_of(rows, cursor, *id);
                        queue.push(Request::user(Command::CheckThreads { ids }));
                    }
                    None => {}
                }
            }
        }
    });
    let offset = output.state.clamped_offset().y;
    ui.ctx()
        .data_mut(|d| d.insert_temp(ui.scope_id().with("list-offset"), offset));
}

fn header(ui: &mut Ui, t: &Tokens, state: &AppState, unread: usize, queue: &mut Vec<Request>) {
    // The search lives in the top bar's pill (the palette, UI-5).
    let title = match (&state.view.search, state.view.mailbox) {
        (Some(query), _) => format!("Search: {query}"),
        (None, _) if state.view.unified => "All inboxes".to_owned(),
        (None, Some(mailbox)) => state
            .mailboxes
            .get(&mailbox)
            .map_or_else(String::new, mailbox_name),
        (None, None) => "dail".to_owned(),
    };
    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(t.m.space.md as i8, 0))
        .show(ui, |ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), t.m.frame.pane_header),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = t.m.space.sm;
                    ui.add(
                        egui::Label::new(ui::text(&t.type_scale.headline_md, title, t.text_strong))
                            .truncate(),
                    );
                    if unread > 0 {
                        ui::count(ui, t, format!("{unread} unread"), true);
                    }
                    // The right side: *Clear* during a search (room for the list toolbar later).
                    if state.view.search.is_some() {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui::secondary(ui, t, "Clear", None, true) {
                                queue.push(Request::user(Command::ClearSearch {}));
                            }
                        });
                    }
                },
            );
            bulk_bar(ui, t, state, queue);
        });
}

/// While threads are checked: how many, and the actions that apply to all of them.
fn bulk_bar(ui: &mut Ui, t: &Tokens, state: &AppState, queue: &mut Vec<Request>) {
    let checked = &state.view.checked;
    if checked.is_empty() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.label(ui::text_at(
            &t.type_scale.body_md,
            600,
            format!("{} checked", checked.len()),
            t.text_strong,
        ));
        let ids = || checked.clone();
        let actions: [(&str, Command); 5] = [
            (
                "Archive",
                Command::ArchiveThreads {
                    ids: ids(),
                    messages: Vec::new(),
                },
            ),
            (
                "Trash",
                Command::TrashThreads {
                    ids: ids(),
                    messages: Vec::new(),
                },
            ),
            (
                "Mark read",
                Command::MarkRead {
                    ids: ids(),
                    messages: Vec::new(),
                },
            ),
            (
                "Mark unread",
                Command::MarkUnread {
                    ids: ids(),
                    messages: Vec::new(),
                },
            ),
            (
                "Star",
                Command::Star {
                    ids: ids(),
                    messages: Vec::new(),
                },
            ),
        ];
        for (label, cmd) in actions {
            if ui::secondary(ui, t, label, None, true) {
                queue.push(Request::user(cmd));
            }
        }
        if ui::secondary(ui, t, "Uncheck all", None, true) {
            queue.push(Request::user(Command::UncheckThreads { ids: Vec::new() }));
        }
    });
}

/// The list footer (UI-5, UI-6): the undo offer while there is one, else the key hints; the
/// number of conversations on the right.
pub fn footer(
    ui: &mut Ui,
    t: &Tokens,
    state: &AppState,
    rows: &Rows,
    undo: Option<&str>,
    queue: &mut Vec<Request>,
) {
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = t.m.space.sm;
        if let Some(label) = undo {
            let keys =
                app_front::keymap::KeyState::hint(&state.settings, app_core::CommandName::Undo);
            if crate::topbar::undo_offer(ui, t, label, keys.as_deref()) {
                queue.push(Request::user(Command::Undo {}));
            }
        } else {
            ui::footer_hint(ui, t, &[("j", "next"), ("k", "previous"), ("x", "select")]);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let n = rows.ids.len();
            let text = format!(
                "{} conversation{}",
                format::thousands(n),
                if n == 1 { "" } else { "s" }
            );
            ui.label(ui::text(
                &t.type_scale.label_sm,
                text.to_uppercase(),
                t.text_muted,
            ));
        });
    });
}

/// What a click on a row asks for: open it; with Ctrl (Cmd), check or uncheck it; with Shift,
/// check every row from the cursor to it.
enum Click {
    Open,
    Toggle,
    Range,
}

/// The rows from `cursor` (or the first row) to `to`, either way round.
fn span_of(rows: &[ThreadId], cursor: Option<ThreadId>, to: ThreadId) -> Vec<ThreadId> {
    let at = |id| rows.iter().position(|r| *r == id);
    let Some(end) = at(to) else {
        return vec![to];
    };
    let start = cursor.and_then(at).unwrap_or(0);
    rows[start.min(end)..=start.max(end)].to_vec()
}

/// How a row is drawn: selection, the open thread, the keyboard cursor and the search words.
struct RowLook<'a> {
    selected: bool,
    checked: bool,
    tag: Option<egui::Color32>,
    open: bool,
    cursor: bool,
    marks: &'a [String],
    /// The newest message's snippet. Untrusted.
    snippet: Option<&'a str>,
}

/// What a screen reader says for a row: what the dot, the star, the clip and the tick show,
/// then senders, subject, size of the conversation and date.
fn row_name(summary: &ThreadSummary<'_>, checked: bool, now_ms: i64) -> String {
    let mut parts: Vec<String> = Vec::new();
    if checked {
        parts.push("checked".to_owned());
    }
    if summary.unread > 0 {
        parts.push("unread".to_owned());
    }
    if summary.flagged {
        parts.push("starred".to_owned());
    }
    if summary.has_attachments {
        parts.push("with attachments".to_owned());
    }
    let senders = format::names(&summary.senders);
    parts.push(if senders.is_empty() {
        "(no sender)".to_owned()
    } else {
        senders
    });
    parts.push(if summary.thread.subject.trim().is_empty() {
        "(no subject)".to_owned()
    } else {
        summary.thread.subject.clone()
    });
    if summary.messages.len() > 1 {
        parts.push(format!("{} messages", summary.messages.len()));
    }
    parts.push(format::short_date(summary.thread.last_date_ms, now_ms));
    parts.join(", ")
}

fn row(
    ui: &mut Ui,
    t: &Tokens,
    summary: &ThreadSummary<'_>,
    look: &RowLook<'_>,
    density: Density,
    now_ms: i64,
) -> Option<Click> {
    let (selected, open, cursor) = (look.selected, look.open, look.cursor);
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), density.height(t)),
        Sense::click(),
    );
    let name = row_name(summary, look.checked, now_ms);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, true, selected || open, &name)
    });
    ui::row_frame(
        ui,
        t,
        rect,
        ui::RowState {
            hovered: response.hovered(),
            selected: open || selected || look.checked,
            cursor,
            hairline: true,
        },
    );
    let unread = summary.unread > 0;
    let body = &t.type_scale.body_md;
    let sender_text = {
        let senders = format::names(&summary.senders);
        if senders.is_empty() {
            "(no sender)".to_owned()
        } else {
            senders
        }
    };
    let sender = if unread {
        ui::text_at(body, 600, sender_text, t.text_strong)
    } else {
        ui::text(body, sender_text, t.text_secondary)
    };
    let subject = if summary.thread.subject.trim().is_empty() {
        "(no subject)"
    } else {
        summary.thread.subject.as_str()
    };
    let subject_color = if unread { t.text } else { t.text_secondary };
    let subject_job = ui::marked(t, subject, look.marks, ui::font(body), subject_color);
    let snippet_job = look.snippet.map(|s| {
        ui::marked(
            t,
            s,
            look.marks,
            ui::font(&t.type_scale.body_sm),
            t.text_muted,
        )
    });
    // Cozy and compact rows: the snippet after the subject, muted, as far as there is room.
    let subject_line = |ui: &mut Ui, subject: egui::text::LayoutJob| {
        ui.add(egui::Label::new(subject).truncate().selectable(false));
        if density != Density::Preview
            && let Some(snippet) = snippet_job.clone()
        {
            ui.add(egui::Label::new(snippet).truncate().selectable(false));
        }
    };
    let date = format::short_date(summary.thread.last_date_ms, now_ms);
    let date_color = if unread { t.text } else { t.text_muted };
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(Vec2::new(t.m.space.md, t.m.space.xs)))
            .layout(Layout::top_down(Align::Min))
            .accessibility_parent(response.id),
    );
    child.spacing_mut().item_spacing = Vec2::new(t.m.space.sm - 2.0, 0.0);
    // The marks before the sender: check, account tag, unread pip.
    let lead = |ui: &mut Ui| {
        if look.checked {
            ui::icon(
                ui,
                Icon::CheckBox,
                t.m.controls.icon_sm,
                t.text_strong,
                true,
            );
        }
        if let Some(tag) = look.tag {
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(t.m.space.xs, t.m.space.md), Sense::hover());
            ui.painter()
                .rect_filled(rect, ui::radius(t.m.radius.sm), tag);
        }
        ui::pip(ui, t, t.text_strong, unread);
    };
    // The marks after: star, attachment, message count, date (right to left).
    let trail = |ui: &mut Ui| {
        ui.label(ui::text(&t.type_scale.label_sm, date.clone(), date_color));
        if summary.messages.len() > 1 {
            ui.label(ui::text(
                &t.type_scale.code_badge,
                summary.messages.len().to_string(),
                t.text_muted,
            ));
        }
        if summary.has_attachments {
            ui::icon(
                ui,
                Icon::AttachFile,
                t.m.controls.icon_xs,
                t.text_muted,
                false,
            );
        }
        if summary.flagged {
            ui::icon(ui, Icon::Star, t.m.controls.icon_xs, t.text_strong, true);
        }
    };
    let line = child.available_width();
    match density {
        Density::Cozy | Density::Preview => {
            let lines = if density == Density::Preview {
                3.0
            } else {
                2.0
            };
            let half = child.available_height() / lines;
            child.allocate_ui_with_layout(
                Vec2::new(line, half),
                Layout::left_to_right(Align::Center),
                |ui| {
                    lead(ui);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        trail(ui);
                        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                            ui.add(egui::Label::new(sender).truncate().selectable(false));
                        });
                    });
                },
            );
            child.allocate_ui_with_layout(
                Vec2::new(line, half),
                Layout::left_to_right(Align::Center),
                |ui| {
                    // Under the sender, past the pip.
                    ui.add_space(t.m.controls.unread_pip + t.m.space.sm);
                    subject_line(ui, subject_job.clone());
                },
            );
            if let Some(snippet) = snippet_job.clone().filter(|_| density == Density::Preview) {
                child.allocate_ui_with_layout(
                    Vec2::new(line, half),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.add_space(t.m.controls.unread_pip + t.m.space.sm);
                        ui.add(egui::Label::new(snippet).truncate().selectable(false));
                    },
                );
            }
        }
        Density::Compact => {
            child.allocate_ui_with_layout(
                Vec2::new(line, child.available_height()),
                Layout::left_to_right(Align::Center),
                |ui| {
                    lead(ui);
                    let column = (line / 3.0).min(t.m.frame.sidebar * 0.625);
                    ui.allocate_ui_with_layout(
                        Vec2::new(column, ui.available_height()),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            ui.set_width(column);
                            ui.add(egui::Label::new(sender).truncate().selectable(false));
                        },
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        trail(ui);
                        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                            subject_line(ui, subject_job.clone());
                        });
                    });
                },
            );
        }
    }
    if !response.clicked() {
        return None;
    }
    let modifiers = ui.input(|i| i.modifiers);
    Some(if modifiers.command {
        Click::Toggle
    } else if modifiers.shift {
        Click::Range
    } else {
        Click::Open
    })
}

/// The rows of the list and how many of them are unread.
#[derive(Debug, Clone, Default)]
pub struct Rows {
    /// [`listing::visible_threads`].
    pub ids: Vec<ThreadId>,
    /// Threads among them with an unread message.
    pub unread: usize,
}

impl Rows {
    fn of(state: &AppState) -> Self {
        let ids = listing::visible_threads(state);
        let unread = ids
            .iter()
            .filter_map(|id| state.threads.get(id))
            .filter(|t| {
                t.messages
                    .iter()
                    .filter_map(|m| state.messages.get(m))
                    .any(|m| !m.flags.seen)
            })
            .count();
        Self { ids, unread }
    }
}

/// [`Rows`], computed once per state (`AppState::revision`): with 100k threads the projection
/// takes ~12 ms, and egui may draw a frame in more than one pass.
#[derive(Debug, Default)]
pub struct RowCache {
    cached: Option<(u64, Rows)>,
}

impl RowCache {
    /// The rows of `state`.
    pub fn of(&mut self, state: &AppState) -> &Rows {
        let revision = state.revision.get();
        if self.cached.as_ref().is_none_or(|(at, _)| *at != revision) {
            self.cached = Some((revision, Rows::of(state)));
        }
        let (_, rows) = self
            .cached
            .get_or_insert_with(|| (revision, Rows::default()));
        rows
    }
}
