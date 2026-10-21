//! The command palette window (Ctrl+K). The rows, flows and ranking are shared with the TUI
//! (`app_front::palette`); this module draws them and reads the keys.

use app_core::Category;
use app_core::{AppState, Command};
use app_front::palette::{Ask, Palette, Row, Target};
use app_theme::Icon;
use egui::{Key, Modifiers, Sense, Ui, Vec2};

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
        let sections = self.inner.sections(&rows);
        let o = &t.m.overlays;
        let modal = egui::Modal::new(id)
            .area(egui::Modal::default_area(id).anchor(
                egui::Align2::CENTER_TOP,
                Vec2::new(0.0, t.m.frame.top_bar + o.palette_offset),
            ))
            .frame(ui::overlay(t).inner_margin(egui::Margin::ZERO))
            .backdrop_color(t.scrim)
            .show(ctx, |ui| {
                ui.set_width(o.palette_width);
                ui.spacing_mut().item_spacing.y = 0.0;
                // The input row: a search icon, the query in `headline_md`, an `Esc` cap.
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(
                        t.m.space.lg as i8,
                        t.m.space.md as i8,
                    ))
                    .show(ui, |ui| {
                        if let Some(step) = self.inner.step() {
                            ui::caption(ui, t, &step.prompt);
                            ui.add_space(t.m.space.xs);
                        }
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = t.m.space.md;
                            ui::icon(ui, Icon::Search, t.m.controls.icon_lg, t.text_muted, false);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui::kbd(ui, t, "Esc");
                                    let hint = self.inner.hint();
                                    let edit = egui::TextEdit::singleline(self.inner.query_mut())
                                        .hint_text(hint)
                                        .frame(egui::Frame::NONE)
                                        .font(ui::font(&t.type_scale.headline_md))
                                        .desired_width(ui.available_width());
                                    let response = ui.add(edit);
                                    if opening || !response.has_focus() {
                                        response.request_focus();
                                    }
                                    if response.changed() {
                                        self.inner.reset_cursor();
                                    }
                                },
                            );
                        });
                    });
                ui::divider(ui, t);
                egui::ScrollArea::vertical()
                    .id_salt("palette")
                    .max_height(o.palette_list_max)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.add_space(t.m.space.xs);
                        if rows.is_empty() {
                            ui::empty_state(ui, t, "Nothing matches.", &[]);
                            ui.add_space(t.m.space.md);
                        }
                        for (i, row) in rows.iter().enumerate() {
                            if let Some(section) = sections.iter().find(|s| s.start == i)
                                && !section.title.is_empty()
                            {
                                ui.add_space(if i == 0 { 0.0 } else { t.m.space.sm });
                                egui::Frame::new()
                                    .inner_margin(egui::Margin::symmetric(
                                        t.m.space.lg as i8,
                                        t.m.space.xs as i8,
                                    ))
                                    .show(ui, |ui| ui::caption(ui, t, section.title));
                            }
                            if row_ui(ui, t, state, row, i == selected, down || up).clicked() {
                                chosen = Some(row.target.clone());
                            }
                        }
                        ui.add_space(t.m.space.xs);
                    });
                ui::divider(ui, t);
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(
                        t.m.space.lg as i8,
                        t.m.space.sm as i8,
                    ))
                    .show(ui, |ui| {
                        if let Some(help) = app_front::palette::Palette::help(&rows, selected) {
                            // The highlighted command's doc comment, as agents read it too.
                            ui.add(
                                egui::Label::new(ui::text(
                                    &t.type_scale.body_sm,
                                    help,
                                    t.text_secondary,
                                ))
                                .wrap(),
                            );
                            ui.add_space(t.m.space.xs);
                        }
                        // Footer: the keys, and the version on the right.
                        ui.horizontal(|ui| {
                            ui.set_min_height(t.m.rows.palette - 2.0 * t.m.space.sm);
                            ui::footer_hint(
                                ui,
                                t,
                                &[
                                    ("↑", "up"),
                                    ("↓", "down"),
                                    ("Enter", "choose"),
                                    ("Esc", "back"),
                                ],
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(ui::text(
                                        &t.type_scale.label_sm,
                                        concat!("dail ", env!("CARGO_PKG_VERSION")),
                                        t.text_muted,
                                    ));
                                },
                            );
                        });
                    });
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

/// The icon of a row: its command's category, a mailbox's role, `Mail` for a thread.
fn row_icon(state: &AppState, row: &Row) -> Icon {
    match &row.target {
        Target::Command(name) => match name.spec().category {
            Category::Mail => Icon::Mail,
            Category::Compose => Icon::Edit,
            Category::Navigation => Icon::ChevronRight,
            Category::Account => Icon::Person,
            Category::App => Icon::Settings,
        },
        Target::Mailbox(id) => {
            crate::theme::role_icon(state.mailboxes.get(id).and_then(|m| m.role))
        }
        Target::Thread(_) => Icon::Mail,
        Target::Search(_) => Icon::Search,
        Target::Choice(_) => Icon::ChevronRight,
    }
}

/// A row (36): icon, title, detail, key caps; the cursor row is an inverse fill with inverse
/// caps.
fn row_ui(
    ui: &mut Ui,
    t: &theme::Tokens,
    state: &AppState,
    row: &Row,
    selected: bool,
    scroll: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), t.m.rows.palette),
        Sense::click(),
    );
    let inset = rect.shrink2(Vec2::new(t.m.space.sm, 1.0));
    if selected {
        ui.painter()
            .rect_filled(inset, ui::radius(t.m.radius.md), t.inverse_bg);
        if scroll {
            response.scroll_to_me(None);
        }
    } else if response.hovered() {
        ui.painter()
            .rect_filled(inset, ui::radius(t.m.radius.md), t.hover);
    }
    let (fg, faint) = if selected {
        (t.inverse_text, t.inverse_text)
    } else {
        (t.text_strong, t.text_muted)
    };
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(Vec2::new(t.m.space.lg, 0.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.spacing_mut().item_spacing.x = t.m.space.md;
    ui::icon(
        &mut child,
        row_icon(state, row),
        t.m.controls.icon_sm,
        faint,
        false,
    );
    child.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if let Some(keys) = &row.keys {
            if selected {
                ui::kbd_inverse(ui, t, keys);
            } else {
                ui::kbd(ui, t, keys);
            }
        }
        if !row.detail.is_empty() {
            ui.label(ui::text(&t.type_scale.body_sm, row.detail.as_str(), faint));
        }
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.add(
                egui::Label::new(ui::text(&t.type_scale.body_md, row.title.as_str(), fg))
                    .truncate()
                    .selectable(false),
            );
        });
    });
    response
}
