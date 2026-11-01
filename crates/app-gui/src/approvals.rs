//! The approvals tray: agents' requests to send a draft, as cards with *Approve*, *Edit* and
//! *Reject* (`ARCHITECTURE.md §4.6`, §7.1). The cards come from `app_front::approvals`; their
//! text is shown as plain labels, never interpreted.

use app_core::{AppState, Command};
use app_front::approvals::{Card, cards};
use app_front::i18n::Key;
use egui::Ui;

use crate::bridge::Request;
use crate::theme::Tokens;
use crate::ui;

/// Renders the tray (the caller shows it only when there are requests).
pub fn show(ui: &mut Ui, t: &Tokens, state: &AppState, queue: &mut Vec<Request>) {
    let cards = cards(state);
    ui::caption(
        ui,
        t,
        &t.f(
            Key::AgentRequest,
            &[(
                "what",
                &match cards.len() {
                    1 => t.s(Key::AMessage).to_owned(),
                    n => t.f(Key::NMessages, &[("n", &n.to_string())]),
                },
            )],
        ),
    );
    egui::ScrollArea::vertical()
        .id_salt("approvals")
        .max_height(t.m.overlays.palette_list_max)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for card in &cards {
                card_ui(ui, t, card, queue);
            }
        });
}

fn card_ui(ui: &mut Ui, t: &Tokens, card: &Card, queue: &mut Vec<Request>) {
    ui::callout(t).fill(t.base).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(ui::text(
                &t.type_scale.headline_md,
                card.subject.as_str(),
                t.text_strong,
            ));
            if !card.from.is_empty() {
                ui.label(ui::text(
                    &t.type_scale.body_sm,
                    t.f(Key::FromName, &[("from", &card.from)]),
                    t.text_muted,
                ));
            }
        });
        for line in &card.recipients {
            ui.label(ui::text(
                &t.type_scale.label_md,
                line.as_str(),
                t.text_secondary,
            ));
        }
        if !card.excerpt.is_empty() {
            ui.label(ui::text(
                &t.type_scale.body_sm,
                card.excerpt.as_str(),
                t.text_secondary,
            ));
        }
        for line in &card.attachments {
            // The full path stays readable: no truncation, plain text.
            let job = ui::icon_text(
                app_theme::Icon::AttachFile,
                line,
                ui::font(&t.type_scale.label_md),
                t.text_strong,
            );
            let name = format!("📎 {line}");
            ui.label(job)
                .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &name));
        }
        ui.add_space(t.m.space.xs);
        ui.horizontal(|ui| {
            if ui::primary(ui, t, t.s(Key::ApproveSend), None, card.ready) {
                queue.push(Request::user(Command::ApproveRequest { id: card.id }));
            }
            if ui::secondary(ui, t, t.s(Key::Edit), None, true) {
                queue.push(Request::user(Command::OpenComposer { id: card.draft }));
            }
            let reject = ui::text(&t.type_scale.body_md, t.s(Key::Reject), t.text_secondary);
            if ui::ghost(ui, t, reject).clicked() {
                queue.push(Request::user(Command::RejectRequest { id: card.id }));
            }
            if !card.ready {
                ui.label(ui::text(
                    &t.type_scale.body_sm,
                    t.s(Key::StillChecked),
                    t.text_muted,
                ));
            }
        });
    });
    ui.add_space(t.m.space.xs + 2.0);
}
