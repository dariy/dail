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
use egui::{RichText, Ui};

use crate::attachments::{self, AttachIo, Previews, Saver};
use crate::bridge::Request;
use crate::document::DocIo;
use crate::images::Images;
use crate::shell::External;
use crate::theme::Palette;
use crate::widgets;

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
pub fn fold_toggle(ui: &mut Ui, p: &Palette, kind: FoldKind, open: bool) -> bool {
    let what = match kind {
        FoldKind::Quote => "quoted text",
        FoldKind::Signature => "signature",
    };
    let label = if open {
        format!("Hide {what}")
    } else {
        format!("··· Show {what}")
    };
    ui.add(egui::Button::new(RichText::new(label).size(12.0).color(p.secondary)).frame(false))
        .clicked()
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
}

/// Renders the open thread (or an empty-state hint).
pub fn show(
    ui: &mut Ui,
    p: &Palette,
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
    } = io;
    let Some(thread) = state.view.thread.and_then(|id| state.threads.get(&id)) else {
        ui.add_space(80.0);
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new("No conversation open")
                    .size(16.0)
                    .color(p.secondary),
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new("j / k to move · Enter to open · Ctrl+K for everything")
                    .color(p.faint),
            );
        });
        return;
    };
    let messages: Vec<&Envelope> = thread
        .messages
        .iter()
        .filter_map(|id| state.messages.get(id))
        .collect();
    let ids = vec![thread.id];
    let last = messages.last().map(|m| m.id);
    ui.add_space(10.0);
    ui.horizontal_wrapped(|ui| {
        let mut push = |cmd| queue.push(Request::user(cmd));
        if let Some(message) = last {
            if widgets::action(ui, p, "↩ Reply", Some("R"), true) {
                push(Command::Reply {
                    message,
                    body: None,
                });
            }
            if widgets::action(ui, p, "Reply all", Some("A"), true) {
                push(Command::ReplyAll {
                    message,
                    body: None,
                });
            }
            if widgets::action(ui, p, "Forward", Some("F"), true) {
                push(Command::Forward {
                    message,
                    body: None,
                });
            }
        }
        ui.add_space(8.0);
        if widgets::action(ui, p, "Archive", Some("E"), true) {
            push(Command::ArchiveThreads {
                ids: ids.clone(),
                messages: Vec::new(),
            });
        }
        if widgets::action(ui, p, "Trash", Some("#"), true) {
            push(Command::TrashThreads {
                ids: ids.clone(),
                messages: Vec::new(),
            });
        }
        let starred = messages.iter().any(|m| m.flags.flagged);
        if widgets::action(
            ui,
            p,
            if starred { "★ Unstar" } else { "☆ Star" },
            Some("S"),
            true,
        ) {
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
        if widgets::action(
            ui,
            p,
            if unread { "Mark read" } else { "Mark unread" },
            Some("U"),
            true,
        ) {
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
        if widgets::action(ui, p, "Close", Some("Esc"), true) {
            push(Command::CloseThread {});
        }
    });
    ui.add_space(12.0);
    let subject = if thread.subject.trim().is_empty() {
        "(no subject)"
    } else {
        &thread.subject
    };
    ui.label(RichText::new(subject).size(22.0).strong().color(p.ink));
    if state.view.focus == Pane::Reader {
        let rect = ui.min_rect();
        ui.painter().hline(
            rect.x_range(),
            rect.bottom() + 4.0,
            egui::Stroke::new(1.0, p.ink),
        );
    }
    ui.add_space(12.0);
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_max_width(ui.available_width().min(760.0));
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
                    };
                    let card = Card {
                        state,
                        thread,
                        message,
                        body,
                        now_ms,
                    };
                    message_card(ui, p, &card, io);
                    ui.add_space(12.0);
                } else {
                    collapsed_row(ui, p, message, body, now_ms, queue);
                    ui.add_space(6.0);
                }
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
    p: &Palette,
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
    let frame = egui::Frame::new()
        .stroke(egui::Stroke::new(1.0, p.border))
        .corner_radius(4)
        .inner_margin(egui::Margin::symmetric(12, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format::age(message.date_ms, now_ms))
                            .font(widgets::mono(11.0))
                            .color(p.faint),
                    );
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        let mut job = egui::text::LayoutJob::default();
                        let style = |color, strong| egui::TextFormat {
                            font_id: egui::FontId::proportional(13.0),
                            color: if strong { p.ink } else { color },
                            ..Default::default()
                        };
                        job.append(&from, 0.0, style(p.ink, !message.flags.seen));
                        job.append(&format!("   {snippet}"), 0.0, style(p.faint, false));
                        ui.add(egui::Label::new(job).truncate().selectable(false));
                    });
                });
            });
        });
    let label = format!("Expand message from {from}");
    let response = ui
        .interact(
            frame.response.rect,
            ui.id().with(("expand", message.id)),
            egui::Sense::click(),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
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

fn message_card(ui: &mut Ui, p: &Palette, card: &Card<'_>, io: ReaderIo<'_>) {
    let Card {
        state,
        thread,
        message,
        body,
        now_ms,
    } = *card;
    let ReaderIo {
        images,
        folds,
        saver,
        previews,
        queue,
        externals,
    } = io;
    egui::Frame::new()
        .fill(p.subtle)
        .corner_radius(4)
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let header = ui.horizontal(|ui| {
                let from = sender(message);
                // The date first, so the truncated sender only gets the space that is left.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!(
                            "{} ({})",
                            format::long_date(message.date_ms),
                            format::age(message.date_ms, now_ms)
                        ))
                        .font(widgets::mono(11.0))
                        .color(p.faint),
                    );
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.add(
                            egui::Label::new(RichText::new(from).strong().color(p.ink))
                                .truncate()
                                .selectable(false),
                        );
                    });
                });
            });
            // In a conversation, a click on the header collapses the message to one line.
            if thread.messages.len() > 1 {
                let label = format!("Collapse message from {}", sender(message));
                let response = ui.interact(
                    header.response.rect,
                    ui.id().with(("collapse", message.id)),
                    egui::Sense::click(),
                );
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label)
                });
                if response.clicked() {
                    queue.push(Request::user(Command::CollapseMessages {
                        ids: vec![message.id],
                    }));
                }
            }
            let to: Vec<String> = message
                .to
                .iter()
                .chain(&message.cc)
                .map(format::full)
                .collect();
            if !to.is_empty() {
                ui.label(RichText::new(format!("to {}", to.join(", "))).color(p.secondary));
            }
            if let Some(mailbox) = state.mailboxes.get(&message.location())
                && mailbox.role != Some(app_core::MailboxRole::Inbox)
            {
                ui.label(
                    RichText::new(app_front::mailbox::mailbox_name(mailbox))
                        .font(widgets::mono(10.5))
                        .color(p.faint),
                );
            }
            if let BodyState::Fetched { blob } = &message.body {
                let button = egui::Button::new(
                    RichText::new("Open original ↗")
                        .size(11.5)
                        .color(p.secondary),
                )
                .frame(false);
                if ui
                    .add(button)
                    .on_hover_text(
                        "A sanitized copy (no scripts, no images, nothing remote) in your browser",
                    )
                    .clicked()
                {
                    externals.push(External::OpenOriginal { blob: blob.clone() });
                }
            }
        });
    ui.add_space(8.0);
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
                images_bar(ui, p, sender, shown, queue);
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
            body_text(ui, p, body, io);
            let io = AttachIo {
                saver,
                previews,
                externals,
            };
            attachments::show(ui, p, state, (message.id, blob), body, io);
        }
        (BodyState::Failed { error }, None) => {
            ui.colored_label(
                ui.visuals().error_fg_color,
                format!("The body could not be downloaded: {error}"),
            );
        }
        _ => {
            ui.label(RichText::new("Loading…").color(p.faint));
        }
    }
}

/// Says that the message has inline images, and whether its sender's are shown.
fn images_bar(ui: &mut Ui, p: &Palette, sender: &str, shown: bool, queue: &mut Vec<Request>) {
    ui.horizontal_wrapped(|ui| {
        let (note, action) = if shown {
            ("Showing the pictures in this message.", "Hide images")
        } else {
            ("Pictures in this message are hidden.", "Show images")
        };
        ui.label(RichText::new(note).size(12.0).color(p.faint));
        let button = egui::Button::new(
            RichText::new(format!("{action} from {sender}"))
                .size(12.0)
                .color(p.secondary),
        )
        .frame(false);
        if ui
            .add(button)
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
    ui.add_space(4.0);
}

fn body_text(ui: &mut Ui, p: &Palette, body: &Body, io: DocIo<'_>) {
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
        crate::document::show(ui, p, document, io);
    } else if body.text.trim().is_empty() {
        ui.label(RichText::new("(no text)").color(p.faint));
    } else {
        let folded = fold::split(&body.text);
        let job = widgets::marked(
            p,
            folded.main,
            marks,
            egui::FontId::proportional(14.0),
            p.ink,
        );
        ui.add(egui::Label::new(job).wrap());
        for (kind, text) in [
            (FoldKind::Signature, folded.signature),
            (FoldKind::Quote, folded.quote),
        ] {
            let Some(text) = text else {
                continue;
            };
            ui.add_space(4.0);
            let open = folds.is_open(message, kind);
            if fold_toggle(ui, p, kind, open) {
                folds.toggle(message, kind);
            }
            if open {
                ui.add(egui::Label::new(RichText::new(text).size(13.5).color(p.secondary)).wrap());
            }
        }
    }
    let links = links(&body.text);
    if !links.is_empty() {
        ui.add_space(8.0);
        widgets::caption(ui, p, &format!("Links ({})", links.len()));
        for url in links {
            // The full target is the label: what you see is where it goes. Opens only on click.
            let response = ui
                .add(
                    egui::Label::new(
                        RichText::new(format!("↗ {url}"))
                            .color(p.secondary)
                            .underline(),
                    )
                    .sense(egui::Sense::click())
                    .truncate(),
                )
                .on_hover_text(format!("Open in your browser:\n{url}"));
            if response.clicked() {
                externals.push(External::OpenUrl(url));
            }
        }
    }
}
