//! The *Pair a device…* dialog (ADR 0017, SRV-QR1): draws `app_front::pairing::PairDialog`.
//! Role and name, then `start_pairing` (as `Background::Pairing`, so its reply comes back
//! here), then the code with *Copy code*, the time left, the address, the fingerprint and the QR
//! code. The QR code is painted as square black modules on a white square with the 4-module
//! quiet zone, white even in a dark theme so that a phone's camera reads it.

use app_core::{AppState, DeviceRole};
use app_front::i18n::Key;
use app_front::pairing::{PairDialog, Stage, countdown, fingerprint_lines};
use app_front::qr::{QUIET_ZONE, QrMatrix, dark_modules};
use egui::{Color32, Ui};

use crate::bridge::{Background, Purpose, Request};
use crate::theme::Tokens;
use crate::ui;

/// The side of the painted QR code, quiet zone included, at least (points).
const QR_SIDE: f32 = 264.0;

/// What the dialog asked for that is not a command.
#[derive(Debug, Default)]
pub struct PairActions {
    /// Copy this text (the code) to the clipboard.
    pub copy: Option<String>,
}

/// The dialog, while it is open.
#[derive(Debug, Default)]
pub struct Pairing {
    dialog: Option<PairDialog>,
}

impl Pairing {
    /// Opens the form.
    pub fn open(&mut self) {
        self.dialog = Some(PairDialog::default());
    }

    /// Opens the dialog for a `start_pairing` submitted elsewhere (the palette).
    pub fn submitted(&mut self, role: DeviceRole, name: &str, state: &AppState) {
        self.dialog = Some(PairDialog::submitted(role, name, state));
    }

    /// Whether it is open.
    pub const fn is_open(&self) -> bool {
        self.dialog.is_some()
    }

    /// The reply to the dialog's `start_pairing`.
    pub fn accept(&mut self, result: Result<&app_core::CommandReceipt, String>, state: &AppState) {
        if let Some(dialog) = &mut self.dialog {
            dialog.accept(result, state);
        }
    }

    /// Renders the dialog.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        t: &Tokens,
        state: &AppState,
        now_ms: i64,
        queue: &mut Vec<Request>,
    ) -> PairActions {
        let mut actions = PairActions::default();
        let Some(dialog) = &mut self.dialog else {
            return actions;
        };
        dialog.sync(state);
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::unique("pair-device"))
            .frame(ui::overlay(t).inner_margin(egui::Margin::same(t.m.space.xl as i8)))
            .backdrop_color(t.scrim)
            .show(ctx, |ui| {
                ui.set_width(t.m.overlays.dialog_width - 2.0 * t.m.space.xl);
                ui.label(ui::text(
                    &t.type_scale.headline_lg,
                    t.s(Key::PairADevice),
                    t.text_strong,
                ));
                ui.add_space(t.m.space.md);
                match dialog.stage().clone() {
                    Stage::Form => form(ui, t, dialog, state, queue, &mut close),
                    Stage::Waiting => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(t.s(Key::PairingWaiting));
                        });
                        if ui::secondary(ui, t, t.s(Key::Close), Some("Esc"), true) {
                            close = true;
                        }
                    }
                    Stage::Failed(error) => {
                        ui.label(ui::text(
                            &t.type_scale.body_md,
                            t.f(
                                Key::PairingFailed,
                                &[("error", &app_front::text::clean(&error))],
                            ),
                            t.text_strong,
                        ));
                        buttons(ui, t, dialog, &mut close);
                    }
                    Stage::Opened(opened) => {
                        ui.horizontal(|ui| {
                            ui.label(ui::text(
                                &t.type_scale.label_md,
                                t.s(Key::PairingCode),
                                t.text_secondary,
                            ));
                            ui.label(
                                egui::RichText::new(opened.code.as_str())
                                    .font(ui::mono(t.type_scale.headline_lg.size))
                                    .color(t.text_strong),
                            );
                            if ui::secondary(ui, t, t.s(Key::CopyCode), None, true) {
                                actions.copy = Some(opened.code.clone());
                            }
                        });
                        let role = match opened.role {
                            DeviceRole::User => t.s(Key::DeviceOfYours),
                            DeviceRole::Agent => t.s(Key::AnAgent),
                        };
                        ui::caption(
                            ui,
                            t,
                            &format!("{} · {role}", app_front::text::clean(&opened.name)),
                        );
                        let remaining = opened.remaining_ms(now_ms);
                        ui.label(if remaining == 0 {
                            t.s(Key::PairingExpired).to_owned()
                        } else {
                            t.f(Key::PairingExpiresIn, &[("time", &countdown(remaining))])
                        });
                        if remaining > 0 {
                            ctx.request_repaint_after(std::time::Duration::from_secs(1));
                        }
                        if let Some(endpoint) = &opened.endpoint {
                            let address = if endpoint.host.contains(':') {
                                format!("[{}]:{}", endpoint.host, endpoint.port)
                            } else {
                                format!("{}:{}", endpoint.host, endpoint.port)
                            };
                            ui.label(t.f(Key::PairingAddress, &[("address", &address)]));
                            ui::caption(ui, t, t.s(Key::PairingFingerprint));
                            for line in fingerprint_lines(&endpoint.fingerprint, 8) {
                                ui.label(
                                    egui::RichText::new(line)
                                        .font(ui::mono(t.type_scale.label_md.size))
                                        .color(t.text),
                                );
                            }
                        }
                        ui.add_space(t.m.space.sm);
                        match &opened.qr {
                            Some(qr) => {
                                ui.vertical_centered(|ui| paint_qr(ui, t, qr));
                                ui::caption(ui, t, t.s(Key::PairingScanHint));
                            }
                            None => {
                                let why = match &opened.link_error {
                                    Some(error) => t.f(
                                        Key::PairingNoLink,
                                        &[("error", &app_front::text::clean(error))],
                                    ),
                                    None => t.s(Key::PairingNoQr).to_owned(),
                                };
                                ui::caption(ui, t, &why);
                            }
                        }
                        buttons(ui, t, dialog, &mut close);
                    }
                }
            });
        if modal.should_close() || close {
            self.dialog = None;
        }
        actions
    }
}

fn form(
    ui: &mut Ui,
    t: &Tokens,
    dialog: &mut PairDialog,
    state: &AppState,
    queue: &mut Vec<Request>,
    close: &mut bool,
) {
    ui.label(ui::text(
        &t.type_scale.label_md,
        t.s(Key::PairWhichKindOfDevice),
        t.text_secondary,
    ));
    for (role, title, detail) in [
        (
            DeviceRole::User,
            Key::DeviceOfYours,
            Key::DeviceOfYoursDetail,
        ),
        (DeviceRole::Agent, Key::AnAgent, Key::AnAgentDetail),
    ] {
        ui.radio_value(&mut dialog.role, role, t.s(title));
        ui.label(ui::text(&t.type_scale.body_sm, t.s(detail), t.text_muted));
    }
    ui.add_space(t.m.space.sm);
    let label = ui.label(ui::text(
        &t.type_scale.label_md,
        t.s(Key::DeviceNameQuestion),
        t.text_secondary,
    ));
    let field = ui
        .add(
            egui::TextEdit::singleline(&mut dialog.name)
                .hint_text(t.s(Key::DeviceNameHint))
                .char_limit(app_front::pairing::MAX_NAME_CHARS)
                .desired_width(f32::INFINITY),
        )
        .labelled_by(label.id);
    if field.changed() {
        // The characters a device name may not have are dropped as they are typed or pasted.
        dialog
            .name
            .retain(|c| !app_front::pairing::forbidden_in_name(c));
    }
    let entered = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    ui.add_space(t.m.space.md);
    ui.horizontal(|ui| {
        if ui::secondary(ui, t, t.s(Key::Cancel), Some("Esc"), true) {
            *close = true;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let pressed = ui::primary(ui, t, t.s(Key::Pair), None, dialog.can_submit());
            if (pressed || entered)
                && let Some(cmd) = dialog.submit(state)
            {
                queue.push(Request {
                    purpose: Purpose::Background(Background::Pairing),
                    cmd,
                });
            }
        });
    });
}

fn buttons(ui: &mut Ui, t: &Tokens, dialog: &mut PairDialog, close: &mut bool) {
    ui.add_space(t.m.space.md);
    ui.horizontal(|ui| {
        if ui::secondary(ui, t, t.s(Key::PairAnother), None, true) {
            dialog.again();
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui::primary(ui, t, t.s(Key::Close), Some("Esc"), true) {
                *close = true;
            }
        });
    });
}

/// Paints `qr` with its quiet zone: whole pixels per module, so every module is a crisp square.
fn paint_qr(ui: &mut Ui, t: &Tokens, qr: &QrMatrix) {
    let (side, dark) = dark_modules(qr, QUIET_ZONE);
    let pixels = ui.ctx().pixels_per_point();
    let wanted = (QR_SIDE * pixels / side.max(1) as f32).ceil().max(4.0);
    let module = wanted / pixels;
    let size = egui::Vec2::splat(module * side as f32);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Image, true, t.s(Key::PairingScanHint))
    });
    let origin = egui::emath::GuiRounding::round_to_pixels(rect.min, pixels);
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, Color32::WHITE);
    for (x, y) in dark {
        let min = origin + egui::vec2(x as f32 * module, y as f32 * module);
        painter.rect_filled(
            egui::Rect::from_min_size(min, egui::Vec2::splat(module)),
            0.0,
            Color32::BLACK,
        );
    }
}
