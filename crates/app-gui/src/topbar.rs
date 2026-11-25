//! The top bar (UI-5): the search pill that opens the palette, the sync state, the link to a
//! dail server (ADR 0017, SRV-6: *Online*, *Connecting…*, *Offline · 3 queued*, with a popover of
//! its details and actions), a status chip with a popover for pending changes, failures and their
//! fixes (a command the server refused offline with *Retry all* and *Dismiss*), and a key hint.
//!
//! The texts come from the shared projections (`app_front::status`, `app_front::server`) and
//! the actions from the shared lists (`app_front::actions`).

use app_core::{AppState, Command, CommandName};
use app_front::actions::Entry;
use app_front::format::Zone;
use app_front::i18n::Key;
use app_front::i18n::{self, Lang};
use app_front::palette::{Ask, Flow};
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

/// What the bar asked for that the shell does.
#[derive(Default)]
pub struct BarActions {
    /// A problem's fix: the password form, or the certificate step.
    pub fix: Option<Fix>,
    /// A palette step to open (*Disconnect…* confirms there).
    pub ask: Option<Ask>,
}

/// Renders the bar with the chrome group's tokens.
pub fn show(
    ui: &mut Ui,
    t: &Tokens,
    state: &AppState,
    (now_ms, zone): (i64, Zone),
    lang: Lang,
    queue: &mut Vec<Request>,
) -> BarActions {
    let mut actions = BarActions::default();
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = t.m.space.md;
        let palette_keys =
            app_front::keymap::KeyState::hint(&state.settings, CommandName::OpenPalette)
                .unwrap_or_else(|| "Ctrl+K".to_owned());
        let width =
            (ui.available_width() * 0.45).clamp(t.m.frame.sidebar, t.m.overlays.palette_width);
        if ui::search_pill(ui, t, t.s(Key::SearchOrRun), &palette_keys, width).clicked() {
            queue.push(Request::user(Command::OpenPalette {}));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = t.m.space.md;
            ui.label(ui::text(
                &t.type_scale.label_sm,
                t.s(Key::MoveHint),
                t.text_muted,
            ));
            let chip = status_chip(ui, t, state, (now_ms, zone), lang, queue);
            let link = link_chip(ui, t, state, lang, queue);
            actions = BarActions {
                fix: chip.fix,
                ask: chip.ask.or(link),
            };
            update_badge(ui, t, state, queue);
            sync_state(ui, t, state, now_ms, lang, queue);
        });
    });
    actions
}

/// Runs an entry of a shared action list: its command goes into the queue, a confirmation
/// step is returned for the palette.
fn run_entry(entry: &Entry, state: &AppState, queue: &mut Vec<Request>) -> Option<Ask> {
    match entry.run.flow(state) {
        Flow::Run(cmd) => {
            queue.push(Request::user(cmd));
            None
        }
        Flow::Ask(step) => Some(step),
        Flow::Password(_) | Flow::Wizard | Flow::Hidden => None,
    }
}

/// The link to the dail server (ADR 0017): its state and the queue, as a chip whose popover
/// shows the server, the pinned certificate, the revision, the last error and the link's
/// actions. Nothing when this profile is not linked.
fn link_chip(
    ui: &mut Ui,
    t: &Tokens,
    state: &AppState,
    lang: Lang,
    queue: &mut Vec<Request>,
) -> Option<Ask> {
    let summary = app_front::server::summary(state, lang)?;
    let (icon, color) = match summary.tone {
        app_front::server::Tone::Online => (Icon::Link, t.text_secondary),
        app_front::server::Tone::Busy => (Icon::Sync, t.text_secondary),
        app_front::server::Tone::Offline => (Icon::CloudOff, t.text_strong),
    };
    let job = ui::icon_text(icon, &summary.text, ui::font(&t.type_scale.label_sm), color);
    let chip = egui::Button::new(job)
        .fill(t.raised)
        .stroke(egui::Stroke::new(t.m.stroke.hairline, t.border))
        .corner_radius(ui::radius(t.m.radius.sm))
        .min_size(egui::Vec2::new(0.0, t.m.controls.badge + t.m.space.xs));
    let response = ui.add(chip);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &summary.text));
    let id = popover_id().with("link");
    let ctx = ui.ctx().clone();
    let mut open = ctx.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    if response.clicked() {
        open = !open;
    }
    let mut ask = None;
    if open {
        let area = egui::Area::new(id.with("area"))
            .order(egui::Order::Foreground)
            .fixed_pos(response.rect.left_bottom() + egui::Vec2::new(0.0, t.m.space.xs))
            .show(&ctx, |ui| {
                ui::overlay(t).show(ui, |ui| {
                    ui.set_max_width(t.m.overlays.dialog_width);
                    ui::caption(ui, t, t.s(Key::DailServer));
                    link_fields(ui, t, state, lang);
                    ui.add_space(t.m.space.sm);
                    ui.horizontal_wrapped(|ui| {
                        for entry in app_front::actions::link_actions(state) {
                            if ui::secondary(ui, t, t.s(entry.label), None, true) {
                                ask = run_entry(&entry, state, queue);
                            }
                        }
                    });
                });
            });
        if area.response.clicked_elsewhere() && !response.clicked() {
            open = false;
        }
        if ask.is_some() {
            open = false;
        }
    }
    ctx.data_mut(|d| d.insert_temp(id, open));
    ask
}

/// The link's details (`app_front::server::details`) as a two-column grid, the fingerprint in
/// monospace.
pub fn link_fields(ui: &mut Ui, t: &Tokens, state: &AppState, lang: Lang) {
    egui::Grid::new(ui.next_auto_id())
        .num_columns(2)
        .spacing([t.m.space.md, t.m.space.xs])
        .show(ui, |ui| {
            for field in app_front::server::details(state, lang) {
                ui.label(ui::text(
                    &t.type_scale.label_md,
                    field.label.as_str(),
                    t.text_muted,
                ));
                let mono = field.label == t.s(Key::PairingFingerprint);
                ui.vertical(|ui| {
                    for line in &field.lines {
                        let text = if mono {
                            egui::RichText::new(line.as_str())
                                .font(ui::mono(t.type_scale.label_md.size))
                                .color(t.text)
                        } else {
                            ui::text(&t.type_scale.body_md, line.as_str(), t.text)
                        };
                        ui.add(egui::Label::new(text).selectable(true));
                    }
                });
                ui.end_row();
            }
        });
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
            // The history backfill can be counted: how far it got.
            if let Some(progress) = status::activity(state, lang).and_then(|a| a.progress) {
                ui.add(
                    egui::ProgressBar::new(progress)
                        .desired_width(t.m.space.xl * 3.0)
                        .desired_height(t.m.space.xs + 2.0)
                        .fill(t.focus),
                );
            }
            r
        })
        .inner;
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &text));
    let sync_keys = app_front::keymap::KeyState::hint(&state.settings, CommandName::SyncNow);
    let hover = match sync_keys {
        Some(keys) => t.f(Key::SyncNowKeys, &[("keys", &keys)]),
        None => t.s(Key::SyncNow).to_owned(),
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
        parts.push(i18n::text(lang, Key::SearchingServer).to_owned());
    }
    (!parts.is_empty()).then(|| (parts.join(" · "), alarming))
}

/// The status chip and its popover with today's texts, hovers and fixes.
fn status_chip(
    ui: &mut Ui,
    t: &Tokens,
    state: &AppState,
    when: (i64, Zone),
    lang: Lang,
    queue: &mut Vec<Request>,
) -> BarActions {
    let Some((text, alarming)) = chip_text(state, lang) else {
        return BarActions::default();
    };
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
    let mut actions = BarActions::default();
    if open {
        let area = egui::Area::new(popover_id().with("area"))
            .order(egui::Order::Foreground)
            .fixed_pos(response.rect.left_bottom() + egui::Vec2::new(0.0, t.m.space.xs))
            .show(&ctx, |ui| {
                ui::overlay(t).show(ui, |ui| {
                    ui.set_max_width(t.m.overlays.dialog_width);
                    actions = details(ui, t, state, when, lang, queue);
                });
            });
        if area.response.clicked_elsewhere() && !response.clicked() {
            open = false;
        }
    }
    ctx.data_mut(|d| d.insert_temp(popover_id(), open));
    actions
}

/// The popover's contents: pending changes with *retry*, server search, problems with fixes (a
/// refused offline command with its error, its time and the shared *Retry all* / *Dismiss*).
fn details(
    ui: &mut Ui,
    t: &Tokens,
    state: &AppState,
    (now_ms, zone): (i64, Zone),
    lang: Lang,
    queue: &mut Vec<Request>,
) -> BarActions {
    let mut actions = BarActions::default();
    let refused = app_front::server::rejections(state, lang, now_ms, zone);
    let small = |text: String, color| ui::text(&t.type_scale.label_md, text, color);
    if let Some((text, failed)) = status::pending_text(state, lang) {
        ui.horizontal(|ui| {
            let label = ui.label(small(text, if failed { t.danger } else { t.text }));
            let errors = status::pending_errors(state);
            if !errors.is_empty() {
                label.on_hover_text(errors.join("\n"));
            }
            if failed
                && ui::ghost(ui, t, small(t.s(Key::FixRetry).to_owned(), t.text_strong))
                    .on_hover_text(t.s(Key::RetryFailed))
                    .clicked()
            {
                queue.push(Request::user(Command::RetryPendingOps { id: None }));
            }
        });
    }
    if status::searching(state) {
        ui.label(small(t.s(Key::SearchingServer).to_owned(), t.text_muted));
    }
    for problem in problems(state, lang) {
        if let Some(Fix::Refused(id)) = problem.fix {
            refusal(
                ui,
                t,
                state,
                &problem,
                refused.iter().find(|r| r.id == id),
                queue,
            );
            continue;
        }
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
                    Fix::Password(_) | Fix::Trust(_) => actions.fix = Some(fix),
                    Fix::Retry(account) => queue.push(Request::user(Command::SyncNow {
                        account: Some(account),
                    })),
                    Fix::Test(id) => queue.push(Request::user(Command::TestAccount { id })),
                    Fix::Refused(_) => {}
                }
            }
        });
    }
    actions
}

/// A command the dail server refused after an offline period: its title, the error and when,
/// and the shared actions (*Retry all*, *Dismiss*).
fn refusal(
    ui: &mut Ui,
    t: &Tokens,
    state: &AppState,
    problem: &status::Problem,
    rejection: Option<&app_front::server::Rejection>,
    queue: &mut Vec<Request>,
) {
    let small = |text: String, color| ui::text(&t.type_scale.label_md, text, color);
    let hover = problem.hint.clone().unwrap_or_default();
    ui.label(small(problem.text.clone(), t.danger))
        .on_hover_text(hover);
    if let Some(rejection) = rejection {
        ui.label(small(
            format!("{} · {}", rejection.error, rejection.when),
            t.text_secondary,
        ));
        ui.horizontal(|ui| {
            for entry in app_front::actions::rejection_actions(state, rejection.id) {
                if ui::ghost(ui, t, small(t.s(entry.label).to_owned(), t.text_strong)).clicked() {
                    run_entry(&entry, state, queue);
                }
            }
        });
    }
}

/// The undo offer in the list footer: “label” and *Undo* with its key. Returns whether it was
/// clicked.
pub fn undo_offer(ui: &mut Ui, t: &Tokens, label: &str, keys: Option<&str>) -> bool {
    ui.label(ui::text(
        &t.type_scale.body_sm,
        format!("“{label}”"),
        t.text_secondary,
    ));
    let button = egui::Button::new(ui::hinted(t, t.s(Key::Undo), keys, t.text_strong)).frame(false);
    ui.add(button).clicked()
}
