//! The approvals tray: agents' requests to send a draft, as cards with *Approve*, *Edit* and
//! *Reject* (`ARCHITECTURE.md §4.6`, §7.1). The cards come from `app_front::approvals`; their
//! text is shown as plain labels, never interpreted.

use app_core::{AppState, Command};
use app_front::approvals::{Card, cards};
use egui::{RichText, Ui};

use crate::bridge::Request;
use crate::theme::Palette;
use crate::widgets;

/// Renders the tray (the caller shows it only when there are requests).
pub fn show(ui: &mut Ui, p: &Palette, state: &AppState, queue: &mut Vec<Request>) {
    let cards = cards(state);
    widgets::caption(
        ui,
        p,
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
                card_ui(ui, p, card, queue);
            }
        });
}

fn card_ui(ui: &mut Ui, p: &Palette, card: &Card, queue: &mut Vec<Request>) {
    egui::Frame::new()
        .fill(p.canvas)
        .stroke(egui::Stroke::new(1.0, p.border_strong))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(&card.subject).strong().color(p.ink));
                if !card.from.is_empty() {
                    ui.label(RichText::new(format!("from {}", card.from)).color(p.faint));
                }
            });
            for line in &card.recipients {
                ui.label(RichText::new(line).color(p.secondary));
            }
            if !card.excerpt.is_empty() {
                ui.label(RichText::new(&card.excerpt).color(p.secondary));
            }
            for line in &card.attachments {
                ui.label(
                    RichText::new(format!("📎 {line}"))
                        .font(widgets::mono(11.5))
                        .color(p.ink),
                );
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if widgets::primary(ui, p, "Approve send", None, card.ready) {
                    queue.push(Request::user(Command::ApproveRequest { id: card.id }));
                }
                if widgets::action(ui, p, "Edit", None, true) {
                    queue.push(Request::user(Command::OpenComposer { id: card.draft }));
                }
                if widgets::action(ui, p, "Reject", None, true) {
                    queue.push(Request::user(Command::RejectRequest { id: card.id }));
                }
                if !card.ready {
                    ui.label(RichText::new("attachments are still being checked").color(p.faint));
                }
            });
        });
    ui.add_space(6.0);
}
