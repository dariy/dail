//! The status bar: sync state, pending changes, errors and the latest undoable action.
//!
//! The texts come from the shared status projections (`app_front::status`).

use app_core::{AccountId, AppState, Command};
use app_front::i18n::Lang;
use app_front::status::{self, Fix, problems, sync_text};
use egui::{RichText, Ui};

use crate::bridge::Request;
use crate::theme::Tokens;
use crate::ui;

/// Renders the bar. `undo` is the label of the latest undoable action, if any. Returns the
/// account whose password the user wants to enter again (a problem's fix).
pub fn show(
    ui: &mut Ui,
    t: &Tokens,
    state: &AppState,
    undo: Option<&str>,
    now_ms: i64,
    lang: Lang,
    queue: &mut Vec<Request>,
) -> Option<AccountId> {
    let mut password = None;
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        let small = |text: String| RichText::new(text).font(ui::mono(11.0));
        ui.label(small(sync_text(state, now_ms, lang)).color(t.text_secondary));
        if status::can_sync(state) {
            let sync = egui::Button::new(small("sync".to_owned()).color(t.text_muted)).frame(false);
            if ui.add(sync).on_hover_text("Sync now (F5)").clicked() {
                queue.push(Request::user(Command::SyncNow { account: None }));
            }
        }
        if let Some((text, failed)) = status::pending_text(state, lang) {
            ui.label(small("·".to_owned()).color(t.text_muted));
            let label = ui.label(small(text).color(if failed {
                ui.visuals().error_fg_color
            } else {
                t.text_secondary
            }));
            let errors = status::pending_errors(state);
            if !errors.is_empty() {
                label.on_hover_text(errors.join("\n"));
            }
            if failed {
                let retry =
                    egui::Button::new(small("retry".to_owned()).color(t.text_strong)).frame(false);
                if ui
                    .add(retry)
                    .on_hover_text("Retry failed changes")
                    .clicked()
                {
                    queue.push(Request::user(Command::RetryPendingOps { id: None }));
                }
            }
        }
        if status::searching(state) {
            ui.label(small("· searching the server…".to_owned()).color(t.text_muted));
        }
        for problem in problems(state, lang) {
            ui.label(small("·".to_owned()).color(t.text_muted));
            let hover = match problem.hint {
                Some(hint) => format!("{hint}\n\n{}", problem.detail),
                None => problem.detail,
            };
            ui.label(small(problem.text).color(ui.visuals().error_fg_color))
                .on_hover_text(hover);
            if let Some(fix) = problem.fix {
                let button =
                    egui::Button::new(small(fix.label(lang)).color(t.text_strong)).frame(false);
                if ui.add(button).clicked() {
                    match fix {
                        Fix::Password(account) => password = Some(account),
                        Fix::Retry(account) => queue.push(Request::user(Command::SyncNow {
                            account: Some(account),
                        })),
                        Fix::Test(id) => queue.push(Request::user(Command::TestAccount { id })),
                    }
                }
            }
        }
        if let Some(label) = undo {
            ui.label(small("·".to_owned()).color(t.text_muted));
            ui.label(RichText::new(format!("“{label}”")).color(t.text_secondary));
            let button =
                egui::Button::new(ui::hinted(t, "Undo", Some("z"), t.text_strong)).frame(false);
            if ui.add(button).clicked() {
                queue.push(Request::user(Command::Undo {}));
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui::kbd(ui, t, "Ctrl+K");
        });
    });
    password
}
