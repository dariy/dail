//! Buttons: primary (inverse), secondary (raised), ghost (no fill) and icon buttons.

use app_theme::Icon;
use egui::{Color32, Response, Stroke, Ui, Vec2, WidgetInfo};

use super::{font, hinted, icon_text, radius};
use crate::theme::Tokens;

/// A solid primary button: `inverse_bg`, `body_md` at 500, an optional key hint, 28 high,
/// radius `md`. Returns whether it was clicked. Its name is the label then the keys.
pub fn primary(ui: &mut Ui, t: &Tokens, label: &str, keys: Option<&str>, enabled: bool) -> bool {
    let text = match keys {
        Some(keys) => format!("{label}   {keys}"),
        None => label.to_owned(),
    };
    let rich = egui::RichText::new(text)
        .font(crate::theme::font_at(&t.type_scale.body_md, 500))
        .color(t.inverse_text);
    let fill = if ui.is_enabled() && enabled {
        t.inverse_bg
    } else {
        t.inverse_bg.gamma_multiply(0.5)
    };
    let button = egui::Button::new(rich)
        .fill(fill)
        .stroke(Stroke::NONE)
        .corner_radius(radius(t.m.radius.md))
        .min_size(Vec2::new(0.0, t.m.controls.button));
    ui.add_enabled(enabled, button).clicked()
}

/// A secondary button: `raised` fill, `border_subtle`, `text`, an optional key hint. Returns
/// whether it was clicked.
pub fn secondary(ui: &mut Ui, t: &Tokens, label: &str, keys: Option<&str>, enabled: bool) -> bool {
    let button = egui::Button::new(hinted(t, label, keys, t.text))
        .fill(t.raised)
        .stroke(Stroke::new(t.m.stroke.hairline, t.border_subtle))
        .corner_radius(radius(t.m.radius.md))
        .min_size(Vec2::new(0.0, t.m.controls.button));
    ui.add_enabled(enabled, button).clicked()
}

/// [`secondary`] with a leading icon; the name is the label then the keys, as in [`secondary`].
pub fn secondary_icon(
    ui: &mut Ui,
    t: &Tokens,
    icon: Icon,
    label: &str,
    keys: Option<&str>,
    enabled: bool,
) -> bool {
    let mut job = icon_text(icon, label, font(&t.type_scale.body_md), t.text);
    if let Some(keys) = keys {
        job.append(
            keys,
            t.m.space.sm,
            egui::TextFormat {
                font_id: font(&t.type_scale.code_badge),
                color: t.text_muted,
                valign: egui::Align::Center,
                ..Default::default()
            },
        );
    }
    let name = format!("{label}{}", keys.unwrap_or_default());
    let button = egui::Button::new(job)
        .fill(t.raised)
        .stroke(Stroke::new(t.m.stroke.hairline, t.border_subtle))
        .corner_radius(radius(t.m.radius.md))
        .min_size(Vec2::new(0.0, t.m.controls.button));
    let response = ui.add_enabled(enabled, button);
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, enabled, &name));
    response.clicked()
}

/// A ghost button: no fill, `hover` on hover; `color` for the text.
pub fn ghost(ui: &mut Ui, t: &Tokens, text: impl Into<egui::WidgetText>) -> Response {
    let button = egui::Button::new(text)
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::NONE)
        .corner_radius(radius(t.m.radius.md))
        .min_size(Vec2::new(0.0, t.m.controls.button_small));
    ui.add(button)
}

/// An icon button (28 × 28, icon 18) whose accessible name is `label`, also its tooltip.
pub fn icon_button(ui: &mut Ui, t: &Tokens, color: Color32, icon: Icon, label: &str) -> Response {
    let side = t.m.controls.icon_button;
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), egui::Sense::click());
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, radius(t.m.radius.md), t.hover);
    }
    super::focus_ring(ui, t, &response, rect);
    super::paint_icon(ui, rect.center(), icon, t.m.controls.icon_md, color, false);
    response.on_hover_text(label)
}
