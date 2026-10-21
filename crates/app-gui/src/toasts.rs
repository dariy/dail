//! Toasts: short-lived notices at the bottom of the window. A scheduled send counts down with
//! *Cancel send*, rejections show the engine's error. The latest undoable action is kept here too
//! and shown in the status bar with *Undo*.

use app_core::{AppState, Command, CommandReceipt, CoreError, DraftId};
use app_front::notice::{self, LastAction};
use egui::{Align2, RichText, Vec2};

use crate::bridge::Request;
use crate::theme::Tokens;
use crate::ui;

/// Seconds a toast stays.
const TOAST_SECS: f64 = 6.0;
/// At most this many toasts are shown.
const MAX_TOASTS: usize = 3;

/// What a toast is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToastKind {
    /// Plain information.
    Info,
    /// A rejected command or a failure.
    Error,
    /// A send waiting for its undo window.
    Sending(DraftId),
}

/// One notice.
#[derive(Debug, Clone, PartialEq)]
pub struct Toast {
    /// The text.
    pub text: String,
    /// What it is about.
    pub kind: ToastKind,
    /// `egui` time (seconds) when it appeared.
    pub since: f64,
}

/// The toasts on screen, and the latest undoable action (shown in the status bar).
#[derive(Debug, Default)]
pub struct Toasts {
    items: Vec<Toast>,
    undo: LastAction,
}

impl Toasts {
    /// The toasts, oldest first.
    pub fn items(&self) -> &[Toast] {
        &self.items
    }

    /// Adds a toast.
    pub fn push(&mut self, text: impl Into<String>, kind: ToastKind, now: f64) {
        self.items.push(Toast {
            text: text.into(),
            kind,
            since: now,
        });
        if self.items.len() > MAX_TOASTS {
            self.items.remove(0);
        }
    }

    /// The label of the latest undoable action of this session, until it is undone.
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.label()
    }

    /// The toast for a user command's receipt, if it deserves one.
    pub fn receipt(&mut self, receipt: &CommandReceipt, now: f64) {
        if let Some(id) = self.undo.observe(receipt) {
            self.push("Sending…", ToastKind::Sending(id), now);
        }
    }

    /// The toast for a rejected command.
    pub fn error(&mut self, title: &str, err: &CoreError, now: f64) {
        self.push(notice::rejection(title, err), ToastKind::Error, now);
    }

    /// Drops expired toasts and renders the rest at the bottom centre.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        t: &Tokens,
        state: &AppState,
        now_ms: i64,
        queue: &mut Vec<Request>,
    ) {
        let now = ctx.input(|i| i.time);
        self.items.retain(|t| match t.kind {
            // A send toast lives as long as the send can be cancelled.
            ToastKind::Sending(id) => notice::send_pending(state, id),
            ToastKind::Error => now - t.since < TOAST_SECS * 2.0,
            ToastKind::Info => now - t.since < TOAST_SECS,
        });
        if self.items.is_empty() {
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
        let mut dismiss = None;
        egui::Area::new(egui::Id::unique("toasts"))
            // Clear of the pane footers and the composer's *Send* row: a toast never covers them.
            .anchor(
                Align2::CENTER_BOTTOM,
                Vec2::new(0.0, -2.0 * t.m.frame.pane_footer),
            )
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                for (i, toast) in self.items.iter().enumerate() {
                    egui::Frame::new()
                        .fill(t.inverse_bg)
                        .corner_radius(6)
                        .inner_margin(egui::Margin::symmetric(14, 8))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let text = match toast.kind {
                                    ToastKind::Sending(id) => {
                                        notice::sending_text(state, id, now_ms)
                                    }
                                    _ => toast.text.clone(),
                                };
                                let color = if toast.kind == ToastKind::Error {
                                    ui.visuals().error_fg_color
                                } else {
                                    t.inverse_text
                                };
                                ui.label(RichText::new(text).color(color));
                                if let ToastKind::Sending(id) = toast.kind {
                                    let button = egui::Button::new(
                                        RichText::new("Cancel send").color(t.inverse_text),
                                    )
                                    .fill(egui::Color32::TRANSPARENT)
                                    .stroke(egui::Stroke::new(1.0, t.text_muted));
                                    if ui.add(button).clicked() {
                                        queue.push(Request::user(Command::CancelSend { id }));
                                        dismiss = Some(i);
                                    }
                                }
                                if ui::icon_button(
                                    ui,
                                    t,
                                    t.inverse_text,
                                    app_theme::Icon::Close,
                                    "Dismiss",
                                )
                                .clicked()
                                {
                                    dismiss = Some(i);
                                }
                            });
                        });
                    ui.add_space(6.0);
                }
            });
        if let Some(i) = dismiss {
            self.items.remove(i);
        }
    }
}
