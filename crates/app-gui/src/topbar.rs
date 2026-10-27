//! The top bar (UI-5): the search pill that opens the palette, the sync state and a status chip
//! with a popover for pending changes, failures and their fixes, and a key hint.
//!
//! The texts come from the shared status projections (`app_front::status`).

use app_core::{AccountId, AppState, Command, CommandName};
use app_front::i18n::Lang;
use app_front::status::{self, Fix, problems, sync_text};
use app_theme::Icon;
use egui::{Align, Layout, Ui};

use crate::bridge::Request;
use crate::theme::Tokens;
use crate::ui;

/// The popover's open state, in the context's memory.
fn popover_id() -> egui::Id {
    egui::Id::unique("dail-status-popover")
}

/// Renders the bar with the chrome group's tokens. Returns the account whose password the user
/// wants to enter again (a problem's fix).
pub fn show(
    ui: &mut Ui,
    t: &Tokens,
    state: &AppState,
    now_ms: i64,
    lang: Lang,
    queue: &mut Vec<Request>,
) -> Option<AccountId> {
    let mut password = None;
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = t.m.space.md;
        let palette_keys =
            app_front::keymap::KeyState::hint(&state.settings, CommandName::OpenPalette)
                .unwrap_or_else(|| "Ctrl+K".to_owned());
        let width =
            (ui.available_width() * 0.45).clamp(t.m.frame.sidebar, t.m.overlays.palette_width);
        if ui::search_pill(ui, t, "Search or run a command", &palette_keys, width).clicked() {
            queue.push(Request::user(Command::OpenPalette {}));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = t.m.space.md;
            ui.label(ui::text(
                &t.type_scale.label_sm,
                "j / k to move",
                t.text_muted,
            ));
            password = status_chip(ui, t, state, lang, queue);
            update_badge(ui, t, state, queue);
            sync_state(ui, t, state, now_ms, lang, queue);
        });
    });
    password
}

/// `Update 1.2.3` while a new version waits (Wave 7 F1); a click opens the update panel.
fn update_badge(ui: &mut Ui, t: &Tokens, state: &AppState, queue: &mut Vec<Request>) {
    let Some(text) = app_front::updates::badge(state) else {
        return;
    };
    if ui::secondary(ui, t, &text, None, true) {
        queue.push(Request::user(Command::OpenSettings {
            section: Some(app_core::SettingsSection::General),
        }));
    }
}

/// The status dot and "synced 2m ago"; a click runs `sync_now`. Its name is the text.
fn sync_state(
    ui: &mut Ui,
    t: &Tokens,
    state: &AppState,
    now_ms: i64,
    lang: Lang,
    queue: &mut Vec<Request>,
) {
    let text = sync_text(state, now_ms, lang);
    let failed = state
        .accounts
        .values()
        .any(|a| matches!(a.status, app_core::AccountStatus::Failed { .. }));
    let dot = if failed {
        ui::Dot::Failed
    } else if status::can_sync(state) || state.accounts.is_empty() {
        ui::Dot::Idle
    } else {
        ui::Dot::Busy
    };
    let response = ui
        .scope(|ui| {
            ui.spacing_mut().item_spacing.x = t.m.space.xs + 2.0;
            let job = ui::job(&t.type_scale.label_sm, &text, t.text_secondary);
            let r = ui.add(
                egui::Label::new(job)
                    .sense(egui::Sense::click())
                    .selectable(false),
            );
            ui::status_dot(ui, t, dot);
            r
        })
        .inner;
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &text));
    let sync_keys = app_front::keymap::KeyState::hint(&state.settings, CommandName::SyncNow);
    let hover = match sync_keys {
        Some(keys) => format!("Sync now ({keys})"),
        None => "Sync now".to_owned(),
    };
    if response.on_hover_text(hover).clicked() && status::can_sync(state) {
        queue.push(Request::user(Command::SyncNow { account: None }));
    }
}

/// The chip text: pending changes first, then the first problem, then the server search.
fn chip_text(state: &AppState, lang: Lang) -> Option<(String, bool)> {
    let problems = problems(state, lang);
    let mut parts: Vec<String> = Vec::new();
    let mut alarming = !problems.is_empty();
    if let Some((text, failed)) = status::pending_text(state, lang) {
        parts.push(text);
        alarming |= failed;
    }
    if let Some(first) = problems.first() {
        parts.push(first.text.clone());
        if problems.len() > 1 {
            parts.push(format!("+{}", problems.len() - 1));
        }
    }
    if status::searching(state) {
        parts.push("searching the server…".to_owned());
    }
    (!parts.is_empty()).then(|| (parts.join(" · "), alarming))
}

/// The status chip and its popover with today's texts, hovers and fixes.
fn status_chip(
    ui: &mut Ui,
    t: &Tokens,
    state: &AppState,
    lang: Lang,
    queue: &mut Vec<Request>,
) -> Option<AccountId> {
    let (text, alarming) = chip_text(state, lang)?;
    let color = if alarming { t.danger } else { t.text_secondary };
    let icon = if alarming {
        Icon::Warning
    } else {
        Icon::Schedule
    };
    let job = ui::icon_text(icon, &text, ui::font(&t.type_scale.label_sm), color);
    let chip = egui::Button::new(job)
        .fill(t.raised)
        .stroke(egui::Stroke::new(t.m.stroke.hairline, t.border))
        .corner_radius(ui::radius(t.m.radius.sm))
        .min_size(egui::Vec2::new(0.0, t.m.controls.badge + t.m.space.xs));
    let response = ui.add(chip);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &text));
    let ctx = ui.ctx().clone();
    let mut open = ctx
        .data(|d| d.get_temp::<bool>(popover_id()))
        .unwrap_or(false);
    if response.clicked() {
        open = !open;
    }
    let mut password = None;
    if open {
        let area = egui::Area::new(popover_id().with("area"))
            .order(egui::Order::Foreground)
            .fixed_pos(response.rect.left_bottom() + egui::Vec2::new(0.0, t.m.space.xs))
            .show(&ctx, |ui| {
                ui::overlay(t).show(ui, |ui| {
                    ui.set_max_width(t.m.overlays.dialog_width);
                    password = details(ui, t, state, lang, queue);
                });
            });
        if area.response.clicked_elsewhere() && !response.clicked() {
            open = false;
        }
    }
    ctx.data_mut(|d| d.insert_temp(popover_id(), open));
    password
}

/// The popover's contents: pending changes with *retry*, server search, problems with fixes.
fn details(
    ui: &mut Ui,
    t: &Tokens,
    state: &AppState,
    lang: Lang,
    queue: &mut Vec<Request>,
) -> Option<AccountId> {
    let mut password = None;
    let small = |text: String, color| ui::text(&t.type_scale.label_md, text, color);
    if let Some((text, failed)) = status::pending_text(state, lang) {
        ui.horizontal(|ui| {
            let label = ui.label(small(text, if failed { t.danger } else { t.text }));
            let errors = status::pending_errors(state);
            if !errors.is_empty() {
                label.on_hover_text(errors.join("\n"));
            }
            if failed
                && ui::ghost(ui, t, small("retry".to_owned(), t.text_strong))
                    .on_hover_text("Retry failed changes")
                    .clicked()
            {
                queue.push(Request::user(Command::RetryPendingOps { id: None }));
            }
        });
    }
    if status::searching(state) {
        ui.label(small("searching the server…".to_owned(), t.text_muted));
    }
    for problem in problems(state, lang) {
        ui.horizontal(|ui| {
            let hover = match problem.hint {
                Some(hint) => format!("{hint}\n\n{}", problem.detail),
                None => problem.detail,
            };
            ui.label(small(problem.text, t.danger)).on_hover_text(hover);
            if let Some(fix) = problem.fix
                && ui::ghost(ui, t, small(fix.label(lang), t.text_strong)).clicked()
            {
                match fix {
                    Fix::Password(account) => password = Some(account),
                    Fix::Retry(account) => queue.push(Request::user(Command::SyncNow {
                        account: Some(account),
                    })),
                    Fix::Test(id) => queue.push(Request::user(Command::TestAccount { id })),
                }
            }
        });
    }
    password
}

/// The undo offer in the list footer: “label” and *Undo* with its key. Returns whether it was
/// clicked.
pub fn undo_offer(ui: &mut Ui, t: &Tokens, label: &str, keys: Option<&str>) -> bool {
    ui.label(ui::text(
        &t.type_scale.body_sm,
        format!("“{label}”"),
        t.text_secondary,
    ));
    let button = egui::Button::new(ui::hinted(t, "Undo", keys, t.text_strong)).frame(false);
    ui.add(button).clicked()
}
