//! The gallery: the living reference of the theme library. It shows the type scale, every colour
//! role with its contrast, and every component of the kit in its states, for the content and the
//! chrome groups side by side. `cargo run -p app-gui --example gallery [-- --theme <id>]`.
//!
//! Hover, press and focus cannot be forced on egui's own widgets, so the components that paint
//! themselves take their state as an argument and show each one; buttons show rest and disabled.

use app_theme::{Icon, Surface, Theme, contrast};
use egui::{ScrollArea, Sense, Ui, Vec2};

use super::{
    Badge, Dot, Nav, RowState, Trailing, avatar, badge, caption, card, card_base, divider,
    empty_state, field, footer_hint, ghost, icon, icon_button, kbd, kbd_inverse, nav_row, overlay,
    pip, primary, row_frame, search_pill, secondary, secondary_icon, segmented, status_dot, switch,
    tabs, text, toast, toolbar, vdivider,
};
use crate::theme::{self, Tokens};

/// The roles of a surface, by name, as the gallery lists them.
fn roles(s: &Surface) -> [(&'static str, app_theme::Rgba); 19] {
    [
        ("base", s.base),
        ("canvas", s.canvas),
        ("raised", s.raised),
        ("hover", s.hover),
        ("selected", s.selected),
        ("emphasis", s.emphasis),
        ("scrim", s.scrim),
        ("border_subtle", s.border_subtle),
        ("border", s.border),
        ("border_strong", s.border_strong),
        ("focus", s.focus),
        ("text_strong", s.text_strong),
        ("text", s.text),
        ("text_secondary", s.text_secondary),
        ("text_muted", s.text_muted),
        ("inverse_bg", s.inverse_bg),
        ("inverse_text", s.inverse_text),
        ("danger", s.danger),
        ("inverse_danger", s.inverse_danger),
    ]
}

/// The gallery of `theme`: content on the left, chrome on the right.
pub fn show(ui: &mut Ui, theme: &Theme) {
    let t = Tokens::of(theme);
    let chrome = t.chrome();
    ScrollArea::vertical()
        .id_salt("gallery")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.columns(2, |cols| {
                let [left, right] = cols else { return };
                egui::Frame::new()
                    .fill(t.canvas)
                    .inner_margin(egui::Margin::same(16))
                    .show(left, |ui| {
                        group(ui, &t, &theme.content, "Content", &theme.name)
                    });
                egui::Frame::new()
                    .fill(chrome.canvas)
                    .inner_margin(egui::Margin::same(16))
                    .show(right, |ui| {
                        ui.style_mut().visuals = theme::chrome_visuals(&t);
                        group(ui, &chrome, &theme.chrome, "Chrome", &theme.name);
                    });
            });
        });
}

fn group(ui: &mut Ui, t: &Tokens, surface: &Surface, title: &str, theme: &str) {
    ui.set_width(ui.available_width());
    ui.label(text(
        &t.type_scale.headline_lg,
        format!("{theme} — {title}"),
        t.text_strong,
    ));
    ui.add_space(t.m.space.md);
    caption(ui, t, "Type scale");
    let s = t.type_scale;
    for (name, token) in [
        ("headline_xl", s.headline_xl),
        ("headline_lg", s.headline_lg),
        ("headline_md", s.headline_md),
        ("body_lg", s.body_lg),
        ("body_md", s.body_md),
        ("body_sm", s.body_sm),
        ("label_md", s.label_md),
        ("label_sm", s.label_sm),
        ("code_badge", s.code_badge),
    ] {
        ui.label(super::job(
            &token,
            &format!(
                "{name} {}/{} {}",
                token.size, token.line_height, token.weight
            ),
            t.text,
        ));
    }
    ui.add_space(t.m.space.lg);
    caption(ui, t, "Colour roles (contrast on base · canvas · selected)");
    for (name, rgba) in roles(surface) {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(28.0, 16.0), Sense::hover());
            ui.painter().rect_filled(rect, 2.0, theme::color(rgba));
            ui.painter().rect_stroke(
                rect,
                2.0,
                egui::Stroke::new(1.0, t.border),
                egui::StrokeKind::Inside,
            );
            ui.label(text(&t.type_scale.label_md, name, t.text));
            let ratios = [surface.base, surface.canvas, surface.selected]
                .map(|bg| format!("{:.2}", contrast(rgba, bg)))
                .join(" · ");
            ui.label(text(&t.type_scale.label_sm, ratios, t.text_muted));
        });
    }
    ui.add_space(t.m.space.lg);
    caption(ui, t, "Buttons (rest, disabled)");
    for enabled in [true, false] {
        ui.horizontal_wrapped(|ui| {
            primary(ui, t, "Reply", Some("R"), enabled);
            secondary(ui, t, "Archive", Some("E"), enabled);
            secondary_icon(ui, t, Icon::Forward, "Forward", Some("F"), enabled);
            ui.add_enabled_ui(enabled, |ui| {
                ghost(
                    ui,
                    t,
                    text(&t.type_scale.body_md, "Clear", t.text_secondary),
                );
                icon_button(ui, t, t.text_secondary, Icon::Star, "Star");
            });
        });
    }
    ui.add_space(t.m.space.md);
    caption(ui, t, "Keys, badges, pips, avatars, status");
    ui.horizontal_wrapped(|ui| {
        kbd(ui, t, "Ctrl+K");
        kbd(ui, t, "g i");
        egui::Frame::new()
            .fill(t.inverse_bg)
            .inner_margin(egui::Margin::same(4))
            .show(ui, |ui| {
                kbd_inverse(ui, t, "Enter");
            });
        badge(ui, t, "12", Badge::Count);
        badge(ui, t, "3 UNREAD", Badge::Strong);
        badge(ui, t, "ARCHIVE", Badge::Outline);
        pip(ui, t, t.text_strong, true);
        avatar(ui, t, "Alice Liddell", t.m.controls.avatar_sm);
        avatar(ui, t, "Bob", t.m.controls.avatar_md);
        avatar(ui, t, "carol@example.com", t.m.controls.avatar_lg);
        status_dot(ui, t, Dot::Idle);
        status_dot(ui, t, Dot::Busy);
        status_dot(ui, t, Dot::Failed);
        for i in [
            Icon::Inbox,
            Icon::Send,
            Icon::Archive,
            Icon::Delete,
            Icon::Settings,
        ] {
            icon(ui, i, t.m.controls.icon_md, t.text_secondary, false);
        }
        icon(ui, Icon::Star, t.m.controls.icon_md, t.text_strong, true);
    });
    ui.add_space(t.m.space.md);
    caption(ui, t, "Nav rows (rest, open, open + focused)");
    for (open, focused) in [(false, false), (true, false), (true, true)] {
        nav_row(
            ui,
            t,
            &Nav {
                icon: Icon::Inbox,
                label: "Inbox",
                trailing: if open {
                    Trailing::Count(4)
                } else {
                    Trailing::Keys("g i".to_owned())
                },
                open,
                focused,
                name: "Inbox",
            },
        );
    }
    ui.add_space(t.m.space.md);
    caption(ui, t, "List rows (rest, hover, selected, cursor)");
    for state in [
        RowState::default(),
        RowState {
            hovered: true,
            ..RowState::default()
        },
        RowState {
            selected: true,
            ..RowState::default()
        },
        RowState {
            selected: true,
            cursor: true,
            ..RowState::default()
        },
    ] {
        let (rect, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), t.m.rows.compact),
            Sense::hover(),
        );
        row_frame(
            ui,
            t,
            rect,
            RowState {
                hairline: true,
                ..state
            },
        );
        ui.painter().text(
            rect.left_center() + Vec2::new(t.m.space.md, 0.0),
            egui::Align2::LEFT_CENTER,
            "Alice Liddell — Lunch?",
            super::font(&t.type_scale.body_md),
            t.text,
        );
    }
    ui.add_space(t.m.space.md);
    caption(ui, t, "Controls");
    search_pill(ui, t, "Search or run a command", "Ctrl+K", 320.0);
    ui.add_space(t.m.space.sm);
    segmented(ui, t, &["System", "Light", "Dark"], 1);
    ui.add_space(t.m.space.sm);
    ui.horizontal(|ui| {
        switch(ui, t, true, "On");
        switch(ui, t, false, "Off");
    });
    ui.add_space(t.m.space.sm);
    tabs(
        ui,
        t,
        &[
            (Icon::Contrast, "Appearance"),
            (Icon::Keyboard, "Keyboard"),
            (Icon::Settings, "General"),
            (Icon::Person, "Accounts"),
        ],
        0,
    );
    ui.add_space(t.m.space.sm);
    toolbar(ui, t, |ui| {
        secondary(ui, t, "Reply", Some("R"), true);
        vdivider(ui, t);
        icon_button(ui, t, t.text_secondary, Icon::Archive, "Archive");
        icon_button(ui, t, t.text_secondary, Icon::Delete, "Trash");
    });
    ui.add_space(t.m.space.md);
    caption(ui, t, "Frames");
    card(t).show(ui, |ui| {
        ui.label(text(&t.type_scale.body_md, "A card on raised", t.text));
    });
    ui.add_space(t.m.space.sm);
    card_base(t).show(ui, |ui| {
        ui.label(text(&t.type_scale.body_md, "A card on base", t.text));
    });
    ui.add_space(t.m.space.sm);
    field(t).show(ui, |ui| {
        ui.label(text(&t.type_scale.body_md, "A field", t.text_muted));
    });
    ui.add_space(t.m.space.sm);
    overlay(t).show(ui, |ui| {
        ui.label(text(&t.type_scale.headline_md, "An overlay", t.text_strong));
    });
    ui.add_space(t.m.space.sm);
    toast(t).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(text(
                &t.type_scale.body_md,
                "Archived 1 conversation",
                t.inverse_text,
            ));
            icon(
                ui,
                Icon::Error,
                t.m.controls.icon_sm,
                t.inverse_danger,
                false,
            );
            ui.label(text(&t.type_scale.body_md, "Send failed", t.inverse_danger));
        });
    });
    ui.add_space(t.m.space.md);
    divider(ui, t);
    footer_hint(ui, t, &[("j k", "move"), ("x", "select")]);
    empty_state(
        ui,
        t,
        "No conversation open",
        &[("j k", "move"), ("Enter", "open"), ("Ctrl+K", "everything")],
    );
}
