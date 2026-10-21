//! The reader: the open conversation, oldest message first, with its bodies.
//!
//! Bodies come from the shared body tracking (`app_front::body`): the shell submits its
//! background `get_thread {include_body: true}`. Everything shown here is email content and is
//! only displayed, never interpreted.

use std::collections::HashSet;

use app_core::document::FoldKind;
use app_core::{AppState, BodyState, Command, Envelope, MessageId, Pane, Thread, listing};
use app_front::body::{Bodies, Body, links};
use app_front::{fold, format};
use app_theme::Icon;
use egui::Ui;

use crate::attachments::{self, AttachIo, Previews, Saver};
use crate::bridge::Request;
use crate::document::DocIo;
use crate::images::Images;
use crate::shell::External;
use crate::theme::Tokens;
use crate::ui;

/// The folds (quoted text, signatures) the user opened, per message. Presentation only, like
/// scrolling: not part of `AppState`.
#[derive(Debug, Default)]
pub struct Folds(HashSet<(MessageId, FoldKind)>);

impl Folds {
    /// Whether the user opened `message`'s fold of this kind.
    pub fn is_open(&self, message: MessageId, kind: FoldKind) -> bool {
        self.0.contains(&(message, kind))
    }

    /// Opens or closes it.
    pub fn toggle(&mut self, message: MessageId, kind: FoldKind) {
        if !self.0.remove(&(message, kind)) {
            self.0.insert((message, kind));
        }
    }
}

/// The button that opens or closes a fold; whether it was clicked.
pub fn fold_toggle(ui: &mut Ui, t: &Tokens, kind: FoldKind, open: bool) -> bool {
    let what = match kind {
        FoldKind::Quote => "quoted text",
        FoldKind::Signature => "signature",
    };
    let (label, icon) = if open {
        (format!("Hide {what}"), Icon::UnfoldLess)
    } else {
        (format!("Show {what}"), Icon::UnfoldMore)
    };
    let job = ui::icon_text(
        icon,
        &label,
        ui::font(&t.type_scale.body_sm),
        t.text_secondary,
    );
    let response = ui::ghost(ui, t, job);
    // The name keeps the dots it always had.
    let name = if open {
        label
    } else {
        format!("··· {label}")
    };
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
    response.clicked()
}

/// Where the reader puts what the user asks for, and its caches.
pub struct ReaderIo<'a> {
    /// Inline images of the open conversation.
    pub images: &'a mut Images,
    /// The folds the user opened.
    pub folds: &'a mut Folds,
    /// The folder dialog for saving attachments.
    pub saver: &'a mut Saver,
    /// Attachment previews.
    pub previews: &'a mut Previews,
    /// Commands for the session.
    pub queue: &'a mut Vec<Request>,
    /// Requests outside the engine.
    pub externals: &'a mut Vec<External>,
    /// The list's rows, for "n of N" and the up and down buttons.
    pub rows: &'a [app_core::ThreadId],
    /// A palette step the toolbar asks for (*Move…*).
    pub ask: &'a mut Option<app_front::palette::Ask>,
    /// Scroll the conversation to its end this frame (a reply composer opened under it).
    pub scroll_to_end: bool,
}

/// Renders the open thread (or an empty-state hint).
pub fn show(
    ui: &mut Ui,
    t: &Tokens,
    state: &AppState,
    bodies: &Bodies,
    now_ms: i64,
    io: ReaderIo<'_>,
) {
    let ReaderIo {
        images,
        folds,
        saver,
        previews,
        queue,
        externals,
        rows,
        ask,
        scroll_to_end,
    } = io;
    let Some(thread) = state.view.thread.and_then(|id| state.threads.get(&id)) else {
        ui.add_space(t.m.frame.top_bar + t.m.space.xl);
        ui::empty_state(
            ui,
            t,
            "No conversation open",
            &[
                ("j", "next"),
                ("k", "previous"),
                ("Enter", "open"),
                ("Ctrl+K", "everything"),
            ],
        );
        return;
    };
    let messages: Vec<&Envelope> = thread
        .messages
        .iter()
        .filter_map(|id| state.messages.get(id))
        .collect();
    let tool = Toolbar {
        state,
        thread: thread.id,
        messages: &messages,
        rows,
    };
    if let Some(step) = toolbar(ui, t, &tool, queue) {
        *ask = Some(step);
    }
    ui.add_space(t.m.space.lg);
    let subject = if thread.subject.trim().is_empty() {
        "(no subject)"
    } else {
        &thread.subject
    };
    ui.add(egui::Label::new(ui::job(&t.type_scale.headline_xl, subject, t.text_strong)).wrap());
    if state.view.focus == Pane::Reader {
        let rect = ui.min_rect();
        ui.painter().hline(
            rect.x_range(),
            rect.bottom() + t.m.space.xs,
            egui::Stroke::new(t.m.stroke.hairline, t.focus),
        );
    }
    ui.add_space(t.m.space.sm);
    meta_line(ui, t, state, thread, messages.len());
    ui.add_space(t.m.space.lg);
    egui::ScrollArea::vertical()
        .id_salt("reader")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_max_width(ui.available_width().min(t.m.frame.text_width_max));
            for message in &messages {
                let body = bodies.get(message.id);
                if listing::is_expanded(state, thread, message) {
                    let io = ReaderIo {
                        images: &mut *images,
                        folds: &mut *folds,
                        saver: &mut *saver,
                        previews: &mut *previews,
                        queue: &mut *queue,
                        externals: &mut *externals,
                        rows,
                        ask: &mut *ask,
                        scroll_to_end: false,
                    };
                    let card = Card {
                        state,
                        thread,
                        message,
                        body,
                        now_ms,
                    };
                    message_card(ui, t, &card, io);
                    ui.add_space(t.m.space.md);
                } else {
                    collapsed_row(ui, t, message, body, now_ms, queue);
                    ui.add_space(t.m.space.xs + 2.0);
                }
            }
            if scroll_to_end {
                ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
            }
        });
}

/// What the toolbar acts on.
struct Toolbar<'a> {
    state: &'a AppState,
    thread: app_core::ThreadId,
    messages: &'a [&'a Envelope],
    rows: &'a [app_core::ThreadId],
}

/// The reader's toolbar (UI-7): *Reply* (primary), *Reply all*, *Forward*; icon buttons for star,
/// read, archive, trash and move, named with their keys; "n of N" with up and down (what `k`
/// and `j` do), and close. It wraps at large text scales. Returns a palette step to open.
fn toolbar(
    ui: &mut Ui,
    t: &Tokens,
    tool: &Toolbar<'_>,
    queue: &mut Vec<Request>,
) -> Option<app_front::palette::Ask> {
    let ids = vec![tool.thread];
    let messages = tool.messages;
    let last = messages.last().map(|m| m.id);
    let mut ask = None;
    ui::toolbar(ui, t, |ui| {
        let mut push = |cmd| queue.push(Request::user(cmd));
        if let Some(message) = last {
            if ui::primary(ui, t, "Reply", Some("R"), true) {
                push(Command::Reply {
                    message,
                    body: None,
                });
            }
            if ui::secondary_icon(ui, t, Icon::ReplyAll, "Reply all", Some("A"), true) {
                push(Command::ReplyAll {
                    message,
                    body: None,
                });
            }
            if ui::secondary_icon(ui, t, Icon::Forward, "Forward", Some("F"), true) {
                push(Command::Forward {
                    message,
                    body: None,
                });
            }
        }
        ui::vdivider(ui, t);
        let starred = messages.iter().any(|m| m.flags.flagged);
        let (icon_color, strong) = (t.text_secondary, t.text_strong);
        let star = ui::icon_button(
            ui,
            t,
            if starred { strong } else { icon_color },
            Icon::Star,
            if starred { "Unstar (S)" } else { "Star (S)" },
        );
        if starred {
            // The starred state: the star filled over the outline.
            ui::paint_icon(
                ui,
                star.rect.center(),
                Icon::Star,
                t.m.controls.icon_md,
                strong,
                true,
            );
        }
        if star.clicked() {
            push(if starred {
                Command::Unstar {
                    ids: ids.clone(),
                    messages: Vec::new(),
                }
            } else {
                Command::Star {
                    ids: ids.clone(),
                    messages: Vec::new(),
                }
            });
        }
        let unread = messages.iter().any(|m| !m.flags.seen);
        let (read_icon, read_name) = if unread {
            (Icon::MarkEmailRead, "Mark read (U)")
        } else {
            (Icon::MarkEmailUnread, "Mark unread (U)")
        };
        if ui::icon_button(ui, t, icon_color, read_icon, read_name).clicked() {
            push(if unread {
                Command::MarkRead {
                    ids: ids.clone(),
                    messages: Vec::new(),
                }
            } else {
                Command::MarkUnread {
                    ids: ids.clone(),
                    messages: Vec::new(),
                }
            });
        }
        if ui::icon_button(ui, t, icon_color, Icon::Archive, "Archive (E)").clicked() {
            push(Command::ArchiveThreads {
                ids: ids.clone(),
                messages: Vec::new(),
            });
        }
        if ui::icon_button(ui, t, icon_color, Icon::Delete, "Trash (#)").clicked() {
            push(Command::TrashThreads {
                ids: ids.clone(),
                messages: Vec::new(),
            });
        }
        if ui::icon_button(ui, t, icon_color, Icon::DriveFileMove, "Move… (V)").clicked()
            && let app_front::palette::Flow::Ask(step) =
                app_front::palette::flow(app_core::CommandName::MoveThreads, tool.state)
        {
            ask = Some(step);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui::icon_button(ui, t, icon_color, Icon::Close, "Close (Esc)").clicked() {
                push(Command::CloseThread {});
            }
            let at = tool.rows.iter().position(|r| *r == tool.thread);
            let next = at.and_then(|i| tool.rows.get(i + 1)).copied();
            let prev = at
                .and_then(|i| i.checked_sub(1))
                .and_then(|i| tool.rows.get(i))
                .copied();
            ui.add_enabled_ui(next.is_some(), |ui| {
                if ui::icon_button(ui, t, icon_color, Icon::ExpandMore, "Next conversation (j)")
                    .clicked()
                    && let Some(id) = next
                {
                    push(Command::OpenThread { id });
                }
            });
            ui.add_enabled_ui(prev.is_some(), |ui| {
                if ui::icon_button(
                    ui,
                    t,
                    icon_color,
                    Icon::ExpandLess,
                    "Previous conversation (k)",
                )
                .clicked()
                    && let Some(id) = prev
                {
                    push(Command::OpenThread { id });
                }
            });
            if let Some(i) = at {
                ui.label(ui::text(
                    &t.type_scale.label_sm,
                    format!("{} of {}", i + 1, tool.rows.len()),
                    t.text_muted,
                ));
            }
        });
    });
    ask
}

/// Chips under the subject: the mailbox (when it is not the inbox), the size of the
/// conversation, the account (with several).
fn meta_line(ui: &mut Ui, t: &Tokens, state: &AppState, thread: &Thread, count: usize) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = t.m.space.xs + 2.0;
        let mailbox = thread
            .messages
            .last()
            .and_then(|m| state.messages.get(m))
            .and_then(|m| state.mailboxes.get(&m.location()));
        if let Some(mailbox) = mailbox
            && mailbox.role != Some(app_core::MailboxRole::Inbox)
        {
            ui::badge(
                ui,
                t,
                &app_front::mailbox::mailbox_name(mailbox).to_uppercase(),
                ui::Badge::Outline,
            );
        }
        let messages = if count == 1 {
            "1 MESSAGE".to_owned()
        } else {
            format!("{count} MESSAGES")
        };
        ui::badge(ui, t, &messages, ui::Badge::Count);
        if state.accounts.len() > 1
            && let Some(account) = state.accounts.get(&thread.account)
        {
            let tag = t.tag(state, account.id);
            let (rect, _) = ui.allocate_exact_size(
                egui::Vec2::new(t.m.space.xs, t.m.controls.badge),
                egui::Sense::hover(),
            );
            ui.painter()
                .rect_filled(rect, ui::radius(t.m.radius.sm), tag);
            ui::badge(ui, t, &account.email, ui::Badge::Outline);
        }
    });
}

fn sender(message: &Envelope) -> String {
    message
        .from
        .first()
        .map_or_else(|| "(no sender)".to_owned(), format::full)
}

/// A collapsed message: sender, the start of its text and the date, on one line. A click
/// expands it.
fn collapsed_row(
    ui: &mut Ui,
    t: &Tokens,
    message: &Envelope,
    body: Option<&Body>,
    now_ms: i64,
    queue: &mut Vec<Request>,
) {
    let from = sender(message);
    let snippet = body
        .map(|b| fold::split(&b.text).main)
        .and_then(|text| text.lines().find(|l| !l.trim().is_empty()))
        .unwrap_or_default()
        .trim()
        .to_owned();
    let name = message
        .from
        .first()
        .map_or("?", |a| a.name.as_deref().unwrap_or(a.email.as_str()));
    let frame = ui::card_base(t)
        .inner_margin(egui::Margin::symmetric(
            t.m.space.md as i8,
            t.m.space.sm as i8,
        ))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = t.m.space.sm;
                ui::avatar(ui, t, name, t.m.controls.avatar_sm);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui::icon(
                        ui,
                        Icon::UnfoldMore,
                        t.m.controls.icon_sm,
                        t.text_muted,
                        false,
                    );
                    ui.label(ui::text(
                        &t.type_scale.label_sm,
                        format::age(message.date_ms, now_ms),
                        t.text_muted,
                    ));
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        let body = &t.type_scale.body_md;
                        let weight = if message.flags.seen { 400 } else { 600 };
                        let mut job = egui::text::LayoutJob::default();
                        job.append(
                            &from,
                            0.0,
                            egui::TextFormat {
                                font_id: crate::theme::font_at(body, weight),
                                color: t.text_strong,
                                ..Default::default()
                            },
                        );
                        job.append(
                            &snippet,
                            t.m.space.md,
                            egui::TextFormat {
                                font_id: ui::font(body),
                                color: t.text_muted,
                                ..Default::default()
                            },
                        );
                        ui.add(egui::Label::new(job).truncate().selectable(false));
                    });
                });
            });
        });
    let label = format!("Expand message from {from}");
    let response = ui
        .interact(
            frame.response.rect,
            ui.scope_id().with(("expand", message.id)),
            egui::Sense::click(),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
    if response.clicked() {
        queue.push(Request::user(Command::ExpandMessages {
            ids: vec![message.id],
        }));
    }
}

/// What a message card shows.
struct Card<'a> {
    state: &'a AppState,
    thread: &'a Thread,
    message: &'a Envelope,
    body: Option<&'a Body>,
    now_ms: i64,
}

fn message_card(ui: &mut Ui, t: &Tokens, card: &Card<'_>, io: ReaderIo<'_>) {
    let Card {
        state,
        thread,
        message,
        body: _,
        now_ms,
    } = *card;
    let ReaderIo {
        images,
        folds,
        saver,
        previews,
        queue,
        externals,
        ..
    } = io;
    ui::card_base(t).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let from = sender(message);
        let header = ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = t.m.space.md;
            let name = message
                .from
                .first()
                .map_or("?", |a| a.name.as_deref().unwrap_or(a.email.as_str()));
            ui::avatar(ui, t, name, t.m.controls.avatar_lg);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = t.m.space.xs / 2.0;
                ui.horizontal(|ui| {
                    // The date first, so the truncated sender only gets the space that is left.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(ui::text(
                            &t.type_scale.label_md,
                            format!(
                                "{} ({})",
                                format::long_date(message.date_ms),
                                format::age(message.date_ms, now_ms)
                            ),
                            t.text_muted,
                        ));
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.add(
                                egui::Label::new(ui::text(
                                    &t.type_scale.headline_md,
                                    from.clone(),
                                    t.text_strong,
                                ))
                                .truncate()
                                .selectable(false),
                            );
                        });
                    });
                });
                let to: Vec<String> = message
                    .to
                    .iter()
                    .chain(&message.cc)
                    .map(format::full)
                    .collect();
                if !to.is_empty() {
                    ui.label(ui::text(
                        &t.type_scale.body_sm,
                        format!("to {}", to.join(", ")),
                        t.text_secondary,
                    ));
                }
            });
        });
        // In a conversation, a click on the header collapses the message to one line.
        if thread.messages.len() > 1 {
            let label = format!("Collapse message from {from}");
            let response = ui.interact(
                header.response.rect,
                ui.scope_id().with(("collapse", message.id)),
                egui::Sense::click(),
            );
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
            if response.clicked() {
                queue.push(Request::user(Command::CollapseMessages {
                    ids: vec![message.id],
                }));
            }
        }
        if let Some(mailbox) = state.mailboxes.get(&message.location())
            && mailbox.role != Some(app_core::MailboxRole::Inbox)
        {
            ui.label(ui::text(
                &t.type_scale.label_sm,
                app_front::mailbox::mailbox_name(mailbox),
                t.text_muted,
            ));
        }
        if let BodyState::Fetched { blob } = &message.body {
            let job = ui::icon_text(
                Icon::OpenInNew,
                "Open original",
                ui::font(&t.type_scale.body_sm),
                t.text_secondary,
            );
            let response = ui::ghost(ui, t, job);
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::Button, true, "Open original ↗")
            });
            if response
                .on_hover_text(
                    "A sanitized copy (no scripts, no images, nothing remote) in your browser",
                )
                .clicked()
            {
                externals.push(External::OpenOriginal { blob: blob.clone() });
            }
        }
        ui.add_space(t.m.space.sm);
        message_body(
            ui,
            t,
            card,
            BodyIo {
                images,
                folds,
                saver,
                previews,
                queue,
                externals,
            },
        );
    });
}

/// Where the body puts what the user asks for.
struct BodyIo<'a> {
    images: &'a mut Images,
    folds: &'a mut Folds,
    saver: &'a mut Saver,
    previews: &'a mut Previews,
    queue: &'a mut Vec<Request>,
    externals: &'a mut Vec<External>,
}

/// The body of an expanded message, its images bar and its attachments.
fn message_body(ui: &mut Ui, t: &Tokens, card: &Card<'_>, io: BodyIo<'_>) {
    let Card {
        state,
        message,
        body,
        ..
    } = *card;
    let BodyIo {
        images,
        folds,
        saver,
        previews,
        queue,
        externals,
    } = io;
    match (&message.body, body) {
        (_, Some(body)) => {
            let blob = match &message.body {
                BodyState::Fetched { blob } => Some(blob.as_str()),
                _ => None,
            };
            let sender = message.from.first().map(|a| a.email.as_str());
            let shown = sender.is_some_and(|s| state.settings.shows_images_from(s));
            if let (Some(document), Some(sender)) = (&body.document, sender)
                && !document.images().is_empty()
            {
                images_bar(ui, t, sender, shown, queue);
            }
            let marks = state
                .view
                .search
                .as_deref()
                .map(app_front::highlight::words)
                .unwrap_or_default();
            let io = DocIo {
                message: message.id,
                folds,
                images: blob.filter(|_| shown).map(|blob| (blob, images)),
                externals: &mut *externals,
                marks: &marks,
            };
            body_text(ui, t, body, io);
            let io = AttachIo {
                saver,
                previews,
                externals,
            };
            attachments::show(ui, t, state, (message.id, blob), body, io);
        }
        (BodyState::Failed { error }, None) => {
            ui.colored_label(
                ui.visuals().error_fg_color,
                format!("The body could not be downloaded: {error}"),
            );
        }
        _ => {
            ui.label(ui::text(&t.type_scale.body_md, "Loading…", t.text_muted));
        }
    }
}

/// Says that the message has inline images, and whether its sender's are shown.
fn images_bar(ui: &mut Ui, t: &Tokens, sender: &str, shown: bool, queue: &mut Vec<Request>) {
    ui.horizontal_wrapped(|ui| {
        let (note, action) = if shown {
            ("Showing the pictures in this message.", "Hide images")
        } else {
            ("Pictures in this message are hidden.", "Show images")
        };
        ui.label(ui::text(&t.type_scale.body_sm, note, t.text_muted));
        let icon = if shown {
            Icon::VisibilityOff
        } else {
            Icon::Visibility
        };
        let label = format!("{action} from {sender}");
        let job = ui::icon_text(
            icon,
            &label,
            ui::font(&t.type_scale.body_sm),
            t.text_secondary,
        );
        let response = ui::ghost(ui, t, job);
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
        if response
            .on_hover_text("Only pictures carried in the message; nothing is downloaded")
            .clicked()
        {
            let sender = sender.to_owned();
            queue.push(Request::user(if shown {
                Command::HideImagesFrom { sender }
            } else {
                Command::ShowImagesFrom { sender }
            }));
        }
    });
    ui.add_space(t.m.space.xs);
}

fn body_text(ui: &mut Ui, t: &Tokens, body: &Body, io: DocIo<'_>) {
    let DocIo {
        message,
        folds,
        images,
        externals,
        marks,
    } = io;
    if let Some(document) = body.document.as_ref().filter(|d| !d.blocks.is_empty()) {
        let io = DocIo {
            message,
            folds: &mut *folds,
            images,
            externals: &mut *externals,
            marks,
        };
        crate::document::show(ui, t, document, io);
    } else if body.text.trim().is_empty() {
        ui.label(ui::text(&t.type_scale.body_lg, "(no text)", t.text_muted));
    } else {
        let folded = fold::split(&body.text);
        let mut job = ui::marked(
            t,
            folded.main,
            marks,
            ui::font(&t.type_scale.body_lg),
            t.text,
        );
        for section in &mut job.sections {
            section.format.line_height = Some(t.type_scale.body_lg.line_height);
        }
        ui.add(egui::Label::new(job).wrap());
        for (kind, text) in [
            (FoldKind::Signature, folded.signature),
            (FoldKind::Quote, folded.quote),
        ] {
            let Some(text) = text else {
                continue;
            };
            ui.add_space(t.m.space.xs);
            let open = folds.is_open(message, kind);
            if fold_toggle(ui, t, kind, open) {
                folds.toggle(message, kind);
            }
            if open {
                ui.add(
                    egui::Label::new(ui::text(&t.type_scale.body_md, text, t.text_secondary))
                        .wrap(),
                );
            }
        }
    }
    let links = links(&body.text);
    if !links.is_empty() {
        ui.add_space(t.m.space.md);
        ui::caption(ui, t, &format!("Links ({})", links.len()));
        for url in links {
            // The full target is the label: what you see is where it goes. Opens only on click.
            let mut job = ui::icon_text(
                Icon::Link,
                &url,
                ui::font(&t.type_scale.body_md),
                t.text_strong,
            );
            if let Some(section) = job.sections.last_mut() {
                section.format.underline = egui::Stroke::new(t.m.stroke.hairline, t.text_strong);
            }
            let name = format!("↗ {url}");
            let response = ui
                .add(egui::Label::new(job).sense(egui::Sense::click()).truncate())
                .on_hover_text(format!("Open in your browser:\n{url}"));
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &name));
            if response.clicked() {
                externals.push(External::OpenUrl(url));
            }
        }
    }
}
