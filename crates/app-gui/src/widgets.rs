//! Small shared widgets: shortcut badges, counters and flat buttons in the monochrome style.

use egui::{Color32, CornerRadius, FontId, RichText, Sense, Stroke, Ui, Vec2};

use crate::theme::Palette;

/// Monospace font for counts, dates and key hints.
pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

/// A keyboard shortcut badge, e.g. `Ctrl+K`.
pub fn kbd(ui: &mut Ui, p: &Palette, keys: &str) {
    let text = RichText::new(keys).font(mono(10.5)).color(p.faint);
    egui::Frame::new()
        .fill(p.subtle)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(2))
        .inner_margin(egui::Margin::symmetric(4, 1))
        .show(ui, |ui| {
            ui.label(text);
        });
}

/// A counter pill: ink fill when `strong`, muted otherwise. Fixed height, centred on the line.
pub fn count(ui: &mut Ui, p: &Palette, n: impl ToString, strong: bool) {
    let (fill, fg) = if strong {
        (p.ink, p.on_ink)
    } else {
        (p.muted, p.secondary)
    };
    let text = n.to_string();
    let galley = ui.painter().layout_no_wrap(text.clone(), mono(10.5), fg);
    let size = Vec2::new(galley.size().x + 10.0, 16.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &text));
    ui.painter().rect_filled(rect, CornerRadius::same(2), fill);
    let pos = rect.center() - galley.size() / 2.0;
    ui.painter().galley(pos, galley, fg);
}

/// A label with a dimmed monospace key hint, e.g. `Reply  R`.
pub fn hinted(
    p: &Palette,
    label: &str,
    keys: Option<&str>,
    color: Color32,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        label,
        0.0,
        egui::TextFormat {
            font_id: FontId::proportional(13.0),
            color,
            ..Default::default()
        },
    );
    if let Some(keys) = keys {
        job.append(
            keys,
            7.0,
            egui::TextFormat {
                font_id: mono(10.5),
                color: p.faint,
                valign: egui::Align::Center,
                ..Default::default()
            },
        );
    }
    job
}

/// A flat toolbar button with an optional key hint. Returns whether it was clicked.
pub fn action(ui: &mut Ui, p: &Palette, label: &str, keys: Option<&str>, enabled: bool) -> bool {
    let button = egui::Button::new(hinted(p, label, keys, p.ink))
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(4))
        .min_size(Vec2::new(0.0, 26.0));
    ui.add_enabled(enabled, button).clicked()
}

/// A solid primary button (ink fill). Returns whether it was clicked.
pub fn primary(ui: &mut Ui, p: &Palette, label: &str, keys: Option<&str>, enabled: bool) -> bool {
    let text = match keys {
        Some(keys) => format!("{label}   {keys}"),
        None => label.to_owned(),
    };
    let button = egui::Button::new(RichText::new(text).color(p.on_ink).strong())
        .fill(p.ink)
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(4))
        .min_size(Vec2::new(0.0, 26.0));
    ui.add_enabled(enabled, button).clicked()
}

/// A section label in small caps style.
pub fn caption(ui: &mut Ui, p: &Palette, text: &str) {
    ui.label(
        RichText::new(text.to_uppercase())
            .font(mono(10.5))
            .color(p.faint),
    );
}

/// A small filled circle (unread pip).
pub fn dot(ui: &mut Ui, color: Color32, visible: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(8.0, 8.0), Sense::hover());
    if visible {
        ui.painter().circle_filled(rect.center(), 3.0, color);
    }
}

/// A frameless glyph button (`×`, `«`) whose accessible name is `label`, also shown on hover.
pub fn icon_button(ui: &mut Ui, color: Color32, glyph: &str, label: &str) -> egui::Response {
    let response = ui
        .add(egui::Button::new(RichText::new(glyph).color(color)).frame(false))
        .on_hover_text(label);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
    response
}

/// `text` with the occurrences of `marks` (search words, `app_front::highlight`) on a muted
/// background.
pub fn marked(
    p: &Palette,
    text: &str,
    marks: &[String],
    font: FontId,
    color: Color32,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    for (piece, marked) in app_front::highlight::pieces(text, marks) {
        job.append(
            piece,
            0.0,
            egui::TextFormat {
                font_id: font.clone(),
                color: if marked { p.ink } else { color },
                background: if marked {
                    p.muted
                } else {
                    Color32::TRANSPARENT
                },
                ..Default::default()
            },
        );
    }
    job
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_words_are_marked() {
        let p = crate::theme::LIGHT;
        let job = marked(
            &p,
            "a Budget!",
            &["budget".to_owned()],
            mono(12.0),
            p.secondary,
        );
        assert_eq!(job.text, "a Budget!");
        let backgrounds: Vec<Color32> = job.sections.iter().map(|s| s.format.background).collect();
        assert_eq!(
            backgrounds,
            [Color32::TRANSPARENT, p.muted, Color32::TRANSPARENT],
            "only the word"
        );
    }
}
