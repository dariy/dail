//! The composer: the draft in `view.composer`, edited in place.
//!
//! Drafts live in the engine (local-first, saved to the server by the runtime). The composer keeps
//! the subject and body being typed in local buffers and writes them back with `update_draft` once
//! typing pauses for a second, the field loses the focus, or before send / close — not on every
//! keystroke, which would flood the undo history. Recipients are chips; typing an address and
//! pressing Enter (or a comma) adds one. Attachments come from the native file dialog (`rfd`,
//! run on its own thread so the UI never blocks).

use app_front::i18n::Key;
use std::sync::mpsc;

use app_core::{Addr, AppState, AttachmentState, Command, Draft, DraftId, Pane, listing};
use app_front::compose::{self, Field};
use app_front::format;
use app_theme::Icon;
use egui::{RichText, Ui};

use crate::bridge::Request;
use crate::theme::Tokens;
use crate::ui;

/// Seconds without typing after which the buffers are written back.
const IDLE_SECS: f64 = 1.0;

/// UI-local composer state.
#[derive(Debug, Default)]
pub struct ComposerState {
    /// The language of the shared texts.
    pub(crate) lang: app_front::i18n::Lang,
    draft: Option<DraftId>,
    /// `updated_ms` of the draft when the buffers were loaded or written.
    base_updated: i64,
    subject: String,
    body: String,
    dirty: bool,
    edited_at: f64,
    inputs: [String; 3],
    show_cc: bool,
    files: Option<mpsc::Receiver<Vec<String>>>,
    /// Everyone in the cached mail, best first, as of a state revision.
    contacts: Option<(u64, Vec<listing::Contact>)>,
    /// The recipient field typed in last: its suggestions stay while the pointer moves to them
    /// (the field loses the focus on the press).
    active: Option<Field>,
    /// Where the suggestions of a field were drawn last frame: a press there takes the focus
    /// from the field, and must pick the suggestion, not add the typed text.
    picks: Option<(Field, egui::Rect)>,
    /// Height of what follows the scrolling fields (attachments, divider and footer, with the
    /// spacing between them) last frame: the fields leave exactly that room. 0 before it is drawn.
    tail_height: f32,
}

/// Suggestions shown under a recipient field.
const SUGGESTIONS: usize = 5;

impl ComposerState {
    fn load(&mut self, draft: &Draft) {
        self.draft = Some(draft.id);
        self.base_updated = draft.updated_ms;
        self.subject.clone_from(&draft.subject);
        self.body.clone_from(&draft.body_text);
        self.dirty = false;
        self.inputs = Default::default();
        self.active = None;
        self.picks = None;
        self.show_cc = !draft.cc.is_empty() || !draft.bcc.is_empty();
    }

    /// Writes typed subject and body back to the draft if they changed.
    pub fn flush(&mut self, state: &AppState, queue: &mut Vec<Request>) {
        let Some(draft) = self.draft.and_then(|id| state.drafts.get(&id)) else {
            return;
        };
        if !self.dirty {
            return;
        }
        self.dirty = false;
        let subject = (self.subject != draft.subject).then(|| self.subject.clone());
        let body = (self.body != draft.body_text).then(|| self.body.clone());
        if subject.is_some() || body.is_some() {
            queue.push(Request::user(Command::UpdateDraft {
                id: draft.id,
                to: None,
                cc: None,
                bcc: None,
                subject,
                body,
            }));
        }
    }

    /// Renders the composer for `view.composer`.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        t: &Tokens,
        state: &AppState,
        now_ms: i64,
        queue: &mut Vec<Request>,
    ) {
        let Some(draft) = state.view.composer.and_then(|id| state.drafts.get(&id)) else {
            self.draft = None;
            return;
        };
        if self.draft != Some(draft.id) || (!self.dirty && draft.updated_ms != self.base_updated) {
            self.load(draft);
        }
        self.receive_files(draft.id, queue);
        let now = ui.input(|i| i.time);
        let locked = draft.is_outgoing();
        if !locked {
            dropped_files(ui, t, draft.id, queue);
        }

        // Header: the title, the save state as a chip, *Close*.
        let header = ui.allocate_ui_with_layout(
            egui::Vec2::new(ui.available_width(), t.m.frame.pane_header),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.spacing_mut().item_spacing.x = t.m.space.sm;
                let title = compose::title(draft);
                ui.label(ui::text(&t.type_scale.headline_md, title, t.text_strong));
                if let Some(error) = &draft.save_error {
                    ui::badge(ui, t, t.s(Key::NotSaved), ui::Badge::Outline)
                        .on_hover_text(t.f(Key::DraftSaveFailed, &[("error", error)]));
                    ui.label(ui::text(
                        &t.type_scale.label_sm,
                        t.f(Key::DraftSaveFailed, &[("error", error)]),
                        t.danger,
                    ));
                } else {
                    let status = compose::draft_status(state, draft, now_ms);
                    ui.label(ui::text(&t.type_scale.label_sm, status, t.text_muted));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui::icon_button(ui, t, t.text_secondary, Icon::Close, t.s(Key::CloseEsc))
                        .clicked()
                    {
                        self.flush(state, queue);
                        queue.push(Request::user(Command::CloseComposer {}));
                    }
                });
            },
        );
        if state.view.focus == Pane::Composer {
            let rect = header.response.rect;
            ui.painter().hline(
                rect.x_range(),
                rect.bottom(),
                egui::Stroke::new(t.m.stroke.hairline, t.focus),
            );
        } else {
            ui::divider(ui, t);
        }
        if let Some(error) = &draft.send_error {
            ui.add_space(t.m.space.sm);
            ui.label(ui::text(
                &t.type_scale.body_md,
                t.f(Key::NotSent, &[("error", error)]),
                t.danger,
            ));
            if let Some(hint) = app_front::status::send_hint(state, draft.account, error, self.lang)
            {
                ui.label(ui::text(&t.type_scale.body_sm, hint, t.text_secondary));
            }
        }

        // The fields and the body scroll between the fixed header and the attachments and footer,
        // so the composer fits any pane (docked under a conversation it may get a third of it)
        // and the attachments, with why one failed, stay in sight.
        // Measured, not estimated: a guess a few points off makes a docked composer's pane grow or
        // shrink by that much every frame.
        let tail = if self.tail_height > 0.0 {
            self.tail_height
        } else {
            t.m.frame.reader_toolbar + 2.0 * ui.spacing().item_spacing.y + t.m.stroke.hairline
        };
        let height = (ui.available_height() - tail).max(t.m.controls.button * 3.0);
        egui::ScrollArea::vertical()
            .id_salt(("compose", draft.id))
            .max_height(height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.fields(ui, t, state, draft, (now, height), queue);
            });
        let fields_bottom = ui.min_rect().bottom();
        if !draft.attachments.is_empty() {
            ui.vertical(|ui| self.attachments(ui, t, draft, queue));
        }

        // Footer: attach on the left; *Discard* and *Send* on the right.
        ui::divider(ui, t);
        ui.horizontal(|ui| {
            ui.set_min_height(t.m.frame.reader_toolbar);
            if locked {
                if draft.send_at.is_some()
                    && !draft.sending
                    && ui::primary(ui, t, t.s(Key::CancelSend), None, true)
                {
                    queue.push(Request::user(Command::CancelSend { id: draft.id }));
                }
                return;
            }
            ui.add_enabled_ui(self.files.is_none(), |ui| {
                if ui::icon_button(
                    ui,
                    t,
                    t.text_secondary,
                    Icon::AttachFile,
                    t.s(Key::AttachFiles),
                )
                .clicked()
                {
                    self.pick_files(ui.ctx(), t.s(Key::AttachFilesTitle));
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let ready = compose::sendable(draft);
                if ui::primary(ui, t, t.s(Key::Send), Some("Ctrl+Enter"), ready) {
                    self.flush(state, queue);
                    queue.push(Request::user(Command::SendDraft { id: draft.id }));
                }
                let discard = ui::text(&t.type_scale.body_md, t.s(Key::Discard), t.text_secondary);
                if ui::ghost(ui, t, discard).clicked() {
                    queue.push(Request::user(Command::DiscardDraft {
                        id: draft.id,
                        keep_server_copy: false,
                    }));
                }
            });
        });
        let measured = ui.min_rect().bottom() - fields_bottom;
        if (measured - self.tail_height).abs() > 0.5 {
            self.tail_height = measured;
            ui.ctx().request_discard("composer footer resized");
        }

        if self.dirty {
            if now - self.edited_at >= IDLE_SECS {
                self.flush(state, queue);
            } else {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_secs_f64(IDLE_SECS));
            }
        }
    }

    /// From, the recipient fields, the subject and the body. `(now, height)`: the frame's time
    /// and the height of the scrolling area, which the body fills.
    fn fields(
        &mut self,
        ui: &mut Ui,
        t: &Tokens,
        state: &AppState,
        draft: &Draft,
        (now, height): (f64, f32),
        queue: &mut Vec<Request>,
    ) {
        let locked = draft.is_outgoing();
        let top = ui.min_rect().top();
        ui.add_enabled_ui(!locked, |ui| {
            let account = state.accounts.get(&draft.account);
            let from = draft.from.as_ref().map_or_else(
                || account.map_or_else(String::new, |a| a.email.clone()),
                |from| from.email.clone(),
            );
            ui.horizontal(|ui| {
                let label = field_label(ui, t, t.s(Key::From));
                // Any connected account's address (MISC-36); the draft moves to that account.
                let aliases = app_front::compose::senders(state, draft.account);
                if aliases.len() > 1 && !locked {
                    // Several addresses to send from: choose one.
                    egui::ComboBox::from_id_salt(("from", draft.id))
                        .selected_text(RichText::new(&from).color(t.text_secondary))
                        .show_ui(ui, |ui| {
                            for email in aliases {
                                let chosen = email.eq_ignore_ascii_case(&from);
                                if ui.selectable_label(chosen, &email).clicked() && !chosen {
                                    queue.push(Request::user(Command::SetDraftFrom {
                                        id: draft.id,
                                        email,
                                    }));
                                }
                            }
                        })
                        .response
                        .labelled_by(label.id);
                } else {
                    ui.label(ui::text(&t.type_scale.body_md, from, t.text_secondary));
                }
                if !self.show_cc {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let cc = ui::text(&t.type_scale.label_md, t.s(Key::CcBcc), t.text_muted);
                        if ui::ghost(ui, t, cc).clicked() {
                            self.show_cc = true;
                        }
                    });
                }
            });
            ui::divider(ui, t);
            for field in Field::ALL {
                if field != Field::To && !self.show_cc {
                    continue;
                }
                self.recipients(ui, t, state, draft, field, queue);
                ui::divider(ui, t);
            }
            ui.horizontal(|ui| {
                let label = field_label(ui, t, t.s(Key::Subject));
                let edit = egui::TextEdit::singleline(&mut self.subject)
                    .font(ui::font(&t.type_scale.body_md))
                    .frame(egui::Frame::NONE)
                    .hint_text(t.s(Key::Subject))
                    .desired_width(f32::INFINITY);
                let response = ui.add(edit).labelled_by(label.id);
                if response.has_focus() {
                    self.active = None;
                }
                if response.changed() {
                    self.dirty = true;
                    self.edited_at = now;
                }
                if response.lost_focus() {
                    self.flush(state, queue);
                }
            });
            ui::divider(ui, t);
            ui.add_space(t.m.space.sm);
            // The body takes the height the fields, the quote note and the attachments leave.
            let used = ui.min_rect().bottom() - top;
            let quote = if draft.quote.is_some() {
                t.m.space.md + t.m.controls.button * 2.0
            } else {
                0.0
            };
            let min = t.m.controls.button * 4.0;
            let fill = (height - used - quote - t.m.space.sm).max(min);
            let edit = egui::TextEdit::multiline(&mut self.body)
                .id_salt(("body", draft.id))
                .font(ui::font(&t.type_scale.body_lg))
                .frame(egui::Frame::NONE)
                .hint_text(t.s(Key::WriteMessage))
                .desired_width(f32::INFINITY)
                .desired_rows(4)
                .min_size(egui::Vec2::new(0.0, fill));
            let response = ui.add(edit);
            if response.has_focus() {
                self.active = None;
            }
            if response.changed() {
                self.dirty = true;
                self.edited_at = now;
            }
            if response.lost_focus() {
                self.flush(state, queue);
            }
            if let Some(quote) = &draft.quote {
                ui.add_space(t.m.space.md);
                ui.label(ui::text(
                    &t.type_scale.body_md,
                    &quote.header,
                    t.text_secondary,
                ));
                ui::caption(ui, t, t.s(Key::OriginalBelow));
            }
        });
    }

    /// The contacts that `typed` could mean, not in `current` yet, best first.
    fn suggestions(&mut self, state: &AppState, typed: &str, current: &[Addr]) -> Vec<Addr> {
        let query = typed.trim().to_lowercase();
        if query.is_empty() {
            return Vec::new();
        }
        let revision = state.revision.get();
        if self.contacts.as_ref().is_none_or(|(at, _)| *at != revision) {
            self.contacts = Some((revision, listing::contacts(state, "", usize::MAX)));
        }
        self.contacts
            .as_ref()
            .map(|(_, all)| {
                all.iter()
                    .filter(|c| c.matches(&query))
                    .filter(|c| {
                        !current
                            .iter()
                            .any(|a| a.email.eq_ignore_ascii_case(&c.email))
                    })
                    .take(SUGGESTIONS)
                    .map(listing::Contact::addr)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn recipients(
        &mut self,
        ui: &mut Ui,
        t: &Tokens,
        state: &AppState,
        draft: &Draft,
        field: Field,
        queue: &mut Vec<Request>,
    ) {
        let current = field.of(draft);
        let mut next: Option<Vec<Addr>> = None;
        let mut focused = false;
        // The label in its column; the chips and the field wrap beside it, under the first chip.
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
            let label = field_label(ui, t, field.label());
            ui.horizontal_wrapped(|ui| {
                for (i, addr) in current.iter().enumerate() {
                    let id = egui::Id::unique(("recipient", draft.id, field as usize, i));
                    let remove = t.f(Key::RemoveName, &[("name", &addr.email)]);
                    let removed = chip(ui, t, id, t.border, &remove, |ui, width| {
                        let name =
                            ui::text(&t.type_scale.body_sm, format::full(addr), t.text_strong);
                        ui.set_max_width(width);
                        ui.add(egui::Label::new(name).truncate())
                            .on_hover_text(&addr.email);
                    });
                    if removed {
                        let mut list = current.to_vec();
                        list.remove(i);
                        next = Some(list);
                    }
                }
                let index = field as usize;
                let Some(input) = self.inputs.get_mut(index) else {
                    return;
                };
                let edit = egui::TextEdit::singleline(input)
                    .font(ui::font(&t.type_scale.body_md))
                    .frame(egui::Frame::NONE)
                    .hint_text(if current.is_empty() {
                        t.s(Key::NameExample)
                    } else {
                        ""
                    })
                    // The rest of the row, after the label and the chips: the field shares their line,
                    // as the sender does. Too little left, and it takes the next line.
                    .desired_width({
                        let rest = ui.available_size_before_wrap().x - t.m.space.sm;
                        if rest >= t.m.frame.rail * 2.0 {
                            rest
                        } else {
                            ui.available_width() - t.m.space.sm
                        }
                    });
                let response = ui.add(edit).labelled_by(label.id);
                if response.has_focus() {
                    focused = true;
                }
                let typed_separator = input.ends_with(',') || input.ends_with(';');
                // A press on a suggestion takes the focus away: that picks the suggestion instead.
                let picking = self.picks.is_some_and(|(at, rect)| {
                    at == field
                        && ui
                            .input(|i| i.pointer.interact_pos())
                            .is_some_and(|pos| rect.contains(pos))
                });
                let submitted = (response.lost_focus() && !picking) || typed_separator;
                if submitted && !input.trim().is_empty() {
                    let added = compose::add_recipients(current, input);
                    let accepted = added.rejected.is_empty();
                    *input = added.rejected;
                    if added.list.is_some() {
                        next = added.list;
                    }
                    if typed_separator && accepted {
                        response.request_focus();
                    }
                }
            });
        });
        let index = field as usize;
        let typed = self.inputs.get(index).cloned().unwrap_or_default();
        if focused {
            self.active = Some(field);
        }
        if self.picks.is_some_and(|(at, _)| at == field) {
            self.picks = None;
        }
        if self.active == Some(field) && next.is_none() {
            let picks = self.suggestions(state, &typed, current);
            if !picks.is_empty() {
                let shown = ui.vertical(|ui| {
                    for addr in picks {
                        let label =
                            ui::text(&t.type_scale.body_sm, format::full(&addr), t.text_secondary);
                        if ui::ghost(ui, t, label)
                            .on_hover_text(t.s(Key::AddRecipient))
                            .clicked()
                        {
                            let mut list = current.to_vec();
                            list.push(addr);
                            next = Some(list);
                            if let Some(input) = self.inputs.get_mut(index) {
                                input.clear();
                            }
                        }
                    }
                });
                self.picks = Some((field, shown.response.rect));
            }
        }
        if let Some(list) = next {
            queue.push(Request::user(field.update(draft, list)));
        }
    }

    /// The attachments as chips, then why any of them failed, on lines of their own (the reason,
    /// such as the size limit, is a sentence: in a chip it ran out of the pane).
    fn attachments(&mut self, ui: &mut Ui, t: &Tokens, draft: &Draft, queue: &mut Vec<Request>) {
        ui.add_space(t.m.space.sm);
        let row = egui::Layout::left_to_right(egui::Align::Center).with_main_wrap(true);
        ui.with_layout(row, |ui| {
            ui.spacing_mut().item_spacing = egui::Vec2::splat(t.m.space.xs);
            for (index, attachment) in (0_u32..).zip(&draft.attachments) {
                let (detail, failed) = match &attachment.state {
                    AttachmentState::Checking => (t.s(Key::Checking).to_owned(), false),
                    AttachmentState::Ready { size, .. } => (format::size(*size), false),
                    AttachmentState::Failed { .. } => (t.s(Key::NativeFailed).to_owned(), true),
                };
                let stroke = if failed { t.danger } else { t.border };
                let id = egui::Id::unique(("attachment", draft.id, index));
                let remove = t.f(Key::RemoveName, &[("name", &attachment.name)]);
                let removed = chip(ui, t, id, stroke, &remove, |ui, width| {
                    // The name gives way to the size or state: it is cut short, they are not.
                    let color = if failed { t.danger } else { t.text_muted };
                    let detail =
                        egui::WidgetText::from(ui::text(&t.type_scale.label_sm, detail, color))
                            .into_galley(
                                ui,
                                Some(egui::TextWrapMode::Extend),
                                f32::INFINITY,
                                egui::TextStyle::Body,
                            );
                    let name_width =
                        (width - detail.size().x - t.m.space.xs).max(t.m.controls.button);
                    ui.scope(|ui| {
                        ui.set_max_width(name_width);
                        ui::icon_label(
                            ui,
                            t,
                            Icon::AttachFile,
                            &attachment.name,
                            t.text_strong,
                            &format!("📎 {}", attachment.name),
                        )
                        .on_hover_text(&attachment.path);
                    });
                    ui.add(egui::Label::new(detail));
                });
                if removed {
                    queue.push(Request::user(Command::RemoveAttachment {
                        draft: draft.id,
                        index,
                    }));
                }
            }
        });
        for attachment in &draft.attachments {
            if let AttachmentState::Failed { error } = &attachment.state {
                ui.add(
                    egui::Label::new(ui::text(
                        &t.type_scale.body_sm,
                        t.f(Key::Failed, &[("error", error)]),
                        t.danger,
                    ))
                    .wrap(),
                );
            }
        }
    }

    fn pick_files(&mut self, ctx: &egui::Context, title: &'static str) {
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        let spawned = std::thread::Builder::new()
            .name("dail-file-dialog".to_owned())
            .spawn(move || {
                let paths = rfd::FileDialog::new()
                    .set_title(title)
                    .pick_files()
                    .unwrap_or_default();
                let paths: Vec<String> = paths
                    .into_iter()
                    .filter_map(|path| path.to_str().map(str::to_owned))
                    .collect();
                let _ = tx.send(paths);
                ctx.request_repaint();
            });
        match spawned {
            Ok(_) => self.files = Some(rx),
            Err(err) => tracing::warn!(error = %err, "could not open the file dialog"),
        }
    }

    fn receive_files(&mut self, draft: DraftId, queue: &mut Vec<Request>) {
        let Some(rx) = &self.files else {
            return;
        };
        match rx.try_recv() {
            Ok(paths) => {
                for path in paths {
                    queue.push(Request::user(Command::AttachFile {
                        draft,
                        path,
                        index: None,
                    }));
                }
                self.files = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => self.files = None,
        }
    }
}

/// Files dropped on the window become attachments of the open draft; while files are dragged
/// over it, a hint says so. Only files whose path is valid UTF-8 can be attached.
fn dropped_files(ui: &Ui, t: &Tokens, draft: DraftId, queue: &mut Vec<Request>) {
    let (hovering, dropped) = ui.input(|i| {
        let paths: Vec<String> = i
            .raw
            .dropped_files
            .iter()
            .filter_map(|f| f.path().to_str().map(str::to_owned))
            .collect();
        (!i.raw.hovered_files.is_empty(), paths)
    });
    for path in dropped {
        queue.push(Request::user(Command::AttachFile {
            draft,
            path,
            index: None,
        }));
    }
    if hovering {
        let rect = ui.ctx().content_rect();
        egui::Area::new(egui::Id::unique("drop-to-attach"))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min)
            .interactable(false)
            .show(ui.ctx(), |ui| {
                // The drop zone: a `border_strong` frame over the scrim.
                egui::Frame::new()
                    .fill(t.canvas.gamma_multiply(0.85))
                    .stroke(egui::Stroke::new(t.m.stroke.indicator, t.border_strong))
                    .show(ui, |ui| {
                        ui.set_min_size(rect.size());
                        ui.centered_and_justified(|ui| {
                            ui.label(ui::text(
                                &t.type_scale.headline_lg,
                                t.s(Key::DropToAttach),
                                t.text_strong,
                            ));
                        });
                    });
            });
    }
}

/// A field's label in the fixed label column: `label_md` in `text_muted`.
fn field_label(ui: &mut Ui, t: &Tokens, text: &str) -> egui::Response {
    ui.add_sized(
        egui::Vec2::new(t.m.frame.rail, t.m.controls.button),
        egui::Label::new(ui::text(&t.type_scale.label_md, text, t.text_muted)),
    )
}

/// A recipient or attachment chip on a wrapping row: `raised`, a hairline in `stroke`, radius
/// `sm`, `content` and a remove button. Returns whether the button was clicked.
///
/// A wrapping row only knows a widget's width after placing it, so a chip remembers its width
/// (under `id`) and starts a new row when it did not fit what this row has left. It is never
/// wider than the row: `content` gets the width left beside the button, and cuts its text short.
fn chip(
    ui: &mut Ui,
    t: &Tokens,
    id: egui::Id,
    stroke: egui::Color32,
    remove: &str,
    content: impl FnOnce(&mut Ui, f32),
) -> bool {
    let row = ui.available_width();
    let left = ui.available_size_before_wrap().x;
    let known = ui.data(|d| d.get_temp::<f32>(id));
    if known.is_some_and(|width| width > left) && left < row - 0.5 {
        ui.end_row();
    }
    let margin = egui::Margin::symmetric(t.m.space.xs as i8 + 2, t.m.space.xs as i8 / 2);
    let inner =
        row - margin.sum().x - 2.0 * t.m.stroke.hairline - t.m.controls.icon_button - t.m.space.xs;
    let mut removed = false;
    let shown = egui::Frame::new()
        .fill(t.raised)
        .stroke(egui::Stroke::new(t.m.stroke.hairline, stroke))
        .corner_radius(ui::radius(t.m.radius.sm))
        .inner_margin(margin)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = t.m.space.xs;
                ui.scope(|ui| content(ui, inner.max(t.m.controls.button)));
                removed = ui::icon_button(ui, t, t.text_muted, Icon::Close, remove).clicked();
            });
        });
    let width = shown.response.rect.width();
    if known.is_none_or(|k| (k - width).abs() > 0.5) {
        ui.data_mut(|d| d.insert_temp(id, width));
        ui.ctx().request_discard("composer chip measured");
    }
    removed
}
