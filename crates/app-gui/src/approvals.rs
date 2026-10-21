//! The approvals tray: agents' requests to send a draft, as cards with *Approve*, *Edit* and
//! *Reject* (`ARCHITECTURE.md §4.6`, §7.1). The cards come from `app_front::approvals`; their
//! text is shown as plain labels, never interpreted.

use app_core::{AppState, Command};
use app_front::approvals::{Card, cards};
use egui::{RichText, Ui};

use crate::bridge::Request;
use crate::theme::Tokens;
use crate::ui;

/// Renders the tray (the caller shows it only when there are requests).
pub fn show(ui: &mut Ui, t: &Tokens, state: &AppState, queue: &mut Vec<Request>) {
    let cards = cards(state);
    ui::caption(
        ui,
        t,
        &format!(
            "An agent wants to send {} — check it before approving",
            match cards.len() {
                1 => "a message".to_owned(),
                n => format!("{n} messages"),
            }
        ),
    );
    egui::ScrollArea::vertical()
        .id_salt("approvals")
        .max_height(240.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for card in &cards {
                card_ui(ui, t, card, queue);
            }
        });
}

fn card_ui(ui: &mut Ui, t: &Tokens, card: &Card, queue: &mut Vec<Request>) {
    egui::Frame::new()
        .fill(t.canvas)
        .stroke(egui::Stroke::new(1.0, t.border_strong))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(&card.subject)
                        .strong()
                        .family(crate::theme::semibold())
                        .color(t.text_strong),
                );
                if !card.from.is_empty() {
                    ui.label(RichText::new(format!("from {}", card.from)).color(t.text_muted));
                }
            });
            for line in &card.recipients {
                ui.label(RichText::new(line).color(t.text_secondary));
            }
            if !card.excerpt.is_empty() {
                ui.label(RichText::new(&card.excerpt).color(t.text_secondary));
            }
            for line in &card.attachments {
                // The full path stays readable: no truncation, plain text.
                let job = ui::icon_text(
                    app_theme::Icon::AttachFile,
                    line,
                    ui::mono(11.5),
                    t.text_strong,
                );
                let name = format!("📎 {line}");
                ui.label(job)
                    .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &name));
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui::primary(ui, t, "Approve send", None, card.ready) {
                    queue.push(Request::user(Command::ApproveRequest { id: card.id }));
                }
                if ui::secondary(ui, t, "Edit", None, true) {
                    queue.push(Request::user(Command::OpenComposer { id: card.draft }));
                }
                if ui::secondary(ui, t, "Reject", None, true) {
                    queue.push(Request::user(Command::RejectRequest { id: card.id }));
                }
                if !card.ready {
                    ui.label(
                        RichText::new("attachments are still being checked").color(t.text_muted),
                    );
                }
            });
        });
    ui.add_space(6.0);
}
