//! The command palette window (Ctrl+K). The rows, flows and ranking are shared with the TUI
//! (`app_front::palette`); this module draws them and reads the keys.

use app_core::{AppState, Command};
use app_front::palette::{Ask, Palette, Row, Target};
use egui::{Key, Modifiers, RichText, Sense, Ui, Vec2};

use crate::bridge::Request;
use crate::theme;
use crate::ui;

/// UI-local palette state.
#[derive(Debug, Default)]
pub struct PaletteState {
    inner: Palette,
    was_open: bool,
}

/// What the palette asked for that is not a command.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PaletteActions {
    /// Open the account wizard.
    pub wizard: bool,
    /// Open the password form for this account.
    pub password: Option<app_core::AccountId>,
}

impl PaletteState {
    /// Starts the palette at a second step (a key binding whose command needs an argument).
    pub fn open_at(&mut self, step: Ask) {
        self.inner.open_at(step);
    }

    /// Chooses `target`, pushing commands into `queue`.
    fn choose(
        &mut self,
        state: &AppState,
        target: Target,
        queue: &mut Vec<Request>,
    ) -> PaletteActions {
        let chosen = self.inner.choose(state, target);
        queue.extend(chosen.commands.into_iter().map(Request::user));
        PaletteActions {
            wizard: chosen.wizard,
            password: chosen.password,
        }
    }

    /// Renders the palette if `view.palette_open`.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        t: &theme::Tokens,
        state: &AppState,
        queue: &mut Vec<Request>,
    ) -> PaletteActions {
        if !state.view.palette_open {
            if self.was_open {
                *self = Self::default();
            }
            return PaletteActions::default();
        }
        let opening = !self.was_open;
        self.was_open = true;
        let mut actions = PaletteActions::default();
        let rows = self.inner.rows(state);
        // Keys are read before the text field sees them.
        let (down, up, enter, escape) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::Enter),
                i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        if down {
            self.inner.move_by(1, rows.len());
        }
        if up {
            self.inner.move_by(-1, rows.len());
        }
        self.inner.clamp(rows.len());
        let mut chosen: Option<Target> = None;
        if enter {
            chosen = rows.get(self.inner.selected()).map(|r| r.target.clone());
        }
        if escape && !self.inner.back() {
            queue.push(Request::user(Command::ClosePalette {}));
        }
        let id = egui::Id::unique("palette");
        let selected = self.inner.selected();
        let modal = egui::Modal::new(id)
            .area(
                egui::Modal::default_area(id)
                    .anchor(egui::Align2::CENTER_TOP, Vec2::new(0.0, 90.0)),
            )
            .frame(
                egui::Frame::new()
                    .fill(t.base)
                    .stroke(egui::Stroke::new(1.0, t.border_strong))
                    .corner_radius(8)
                    .inner_margin(egui::Margin::same(0)),
            )
            .backdrop_color(egui::Color32::from_black_alpha(90))
            .show(ctx, |ui| {
                ui.set_width(560.0);
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(16, 12))
                    .show(ui, |ui| {
                        if let Some(step) = self.inner.step() {
                            ui::caption(ui, t, &step.prompt);
                            ui.add_space(4.0);
                        }
                        let hint = self.inner.hint();
                        let edit = egui::TextEdit::singleline(self.inner.query_mut())
                            .hint_text(hint)
                            .frame(egui::Frame::NONE)
                            .font(egui::FontId::proportional(17.0))
                            .desired_width(f32::INFINITY);
                        let response = ui.add(edit);
                        if opening || !response.has_focus() {
                            response.request_focus();
                        }
                        if response.changed() {
                            self.inner.reset_cursor();
                        }
                    });
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt("palette")
                    .max_height(380.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        if rows.is_empty() {
                            ui.add_space(8.0);
                            ui.label(RichText::new("    Nothing matches.").color(t.text_muted));
                            ui.add_space(8.0);
                        }
                        for (i, row) in rows.iter().enumerate() {
                            if row_ui(ui, t, row, i == selected, down || up).clicked() {
                                chosen = Some(row.target.clone());
                            }
                        }
                    });
                ui.separator();
                if let Some(help) = app_front::palette::Palette::help(&rows, selected) {
                    // The highlighted command's doc comment, as agents read it too.
                    ui.horizontal_wrapped(|ui| {
                        ui.add_space(12.0);
                        ui.label(RichText::new(help).size(12.0).color(t.text_secondary));
                    });
                }
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new("↑↓ to move · Enter to choose · Esc to go back")
                            .font(ui::mono(10.5))
                            .color(t.text_muted),
                    );
                });
                ui.add_space(4.0);
            });
        if modal.backdrop_response.clicked() {
            queue.push(Request::user(Command::ClosePalette {}));
        }
        if let Some(target) = chosen {
            actions = self.choose(state, target, queue);
        }
        actions
    }
}

fn row_ui(
    ui: &mut Ui,
    t: &theme::Tokens,
    row: &Row,
    selected: bool,
    scroll: bool,
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::click());
    if selected {
        ui.painter()
            .rect_filled(rect.shrink2(Vec2::new(6.0, 1.0)), 4.0, t.inverse_bg);
        if scroll {
            response.scroll_to_me(None);
        }
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect.shrink2(Vec2::new(6.0, 1.0)), 4.0, t.hover);
    }
    let (fg, faint) = if selected {
        (t.inverse_text, t.inverse_text)
    } else {
        (t.text_strong, t.text_muted)
    };
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(Vec2::new(18.0, 0.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if let Some(keys) = &row.keys {
            ui.label(RichText::new(keys).font(ui::mono(10.5)).color(faint));
        }
        if !row.detail.is_empty() {
            ui.label(RichText::new(&row.detail).size(11.5).color(faint));
        }
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.add(
                egui::Label::new(RichText::new(&row.title).color(fg))
                    .truncate()
                    .selectable(false),
            );
        });
    });
    response
}
