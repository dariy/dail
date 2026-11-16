//! Copying text out of the message pane: the text selected in labels, and the pane's context
//! menu (*Copy*, *Copy all text*).
//!
//! egui copies a label selection on Ctrl+C by itself. It does not say what is selected, and a
//! right click on a label collapses the selection before a menu could ask. So a plugin keeps the
//! text egui reports when a selection settles (`TextSelectionSettled`), and the menu takes it at
//! the secondary press.

use egui::{Id, Popup, PopupAnchor, PopupKind, Rect, SetOpenCommand, Ui};

use app_front::i18n::Key;

use crate::theme::Tokens;

/// The text of the last settled label selection; empty once a primary press starts another.
#[derive(Debug, Default)]
struct Selected {
    text: String,
}

impl egui::Plugin for Selected {
    fn debug_name(&self) -> &'static str {
        "dail::selection"
    }

    fn input_hook(&mut self, _ctx: &egui::Context, input: &mut egui::RawInput) {
        // A primary press ends the selection or starts a new one, which settles on release.
        let starts = input.events.iter().any(|event| {
            matches!(
                event,
                egui::Event::PointerButton {
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    ..
                }
            )
        });
        if starts {
            self.text.clear();
        }
    }

    fn output_hook(&mut self, _ctx: &egui::Context, output: &mut egui::FullOutput) {
        for command in &output.platform_output.commands {
            if let egui::OutputCommand::TextSelectionSettled(text) = command {
                self.text.clone_from(text);
            }
        }
    }
}

/// The text selected in labels, if any.
pub fn selected(ctx: &egui::Context) -> String {
    ctx.with_plugin(|selected: &mut Selected| selected.text.clone())
        .unwrap_or_default()
}

/// The context menu of a region of selectable text (a message's body): *Copy* the selection and
/// *Copy all text* (`all`). A secondary click anywhere in `rect` opens it at the pointer. The
/// labels keep their own clicks and drags: nothing is laid over them.
pub fn context_menu(ui: &Ui, t: &Tokens, rect: Rect, message: app_core::MessageId, all: &str) {
    let ctx = ui.ctx().clone();
    // Added once: a plugin of a type already there is ignored.
    ctx.add_plugin(Selected::default());
    let id = Id::unique(("copy-menu", message));
    let rect = rect.intersect(ui.clip_rect());
    let layer = ui.layer_id();
    // The context's locks do not nest: read the pointer first, then ask whose layer it is on.
    let (pos, pressed, clicked) = ctx.input(|i| {
        (
            i.pointer.interact_pos(),
            i.pointer.secondary_pressed(),
            i.pointer.secondary_clicked(),
        )
    });
    let here = pos.is_some_and(|pos| rect.contains(pos) && ctx.layer_id_at(pos) == Some(layer));
    if here && pressed {
        // Now, while the selection stands: the press itself collapses it.
        let text = selected(&ctx);
        ctx.data_mut(|data| data.insert_temp(id, text));
    }
    Popup::new(id, ctx.clone(), PopupAnchor::PointerFixed, layer)
        .kind(PopupKind::Menu)
        .layout(egui::Layout::top_down_justified(egui::Align::Min))
        .style(egui::containers::menu::menu_style)
        .gap(0.0)
        .open_memory((here && clicked).then_some(SetOpenCommand::Bool(true)))
        .show(|ui| {
            let text: String = ctx.data(|data| data.get_temp(id)).unwrap_or_default();
            if ui
                .add_enabled(!text.is_empty(), egui::Button::new(t.s(Key::Copy)))
                .clicked()
            {
                ctx.copy_text(text);
                ui.close();
            }
            if ui.button(t.s(Key::CopyAllText)).clicked() {
                ctx.copy_text(all.to_owned());
                ui.close();
            }
        });
}
