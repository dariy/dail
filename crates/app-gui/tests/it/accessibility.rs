//! Accessibility: names for screen readers, the high-contrast themes, text scale and the order
//! Tab moves through a form.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use crate::support::{apply, gui, inbox_of, synced_state};
use app_core::{AppState, Command, SettingKey};
use app_gui::theme;
use egui::Key;
use egui::accesskit::Role;
use egui_kittest::kittest::{By, NodeT, Queryable};

#[test]
fn rows_have_names_that_say_what_the_marks_show() {
    let state = synced_state();
    let (inbox, _) = inbox_of(&state);
    let mut h = gui(state);
    apply(&mut h, Command::OpenMailbox { id: inbox });
    h.run();
    // The unread thread from the fixture: its dot is spoken as "unread".
    let row = h.get(
        By::new()
            .role(Role::Button)
            .predicate(|n| n.label().is_some_and(|l| l.contains("Lunch?"))),
    );
    assert!(
        row.accesskit_node()
            .label()
            .unwrap()
            .starts_with("unread, ")
    );
    // The open mailbox is selected, with its unread count.
    let inbox_row = h.get(By::new().role(Role::Button).label_contains("Inbox, "));
    assert!(
        inbox_row
            .accesskit_node()
            .label()
            .unwrap()
            .ends_with(" unread")
    );
    assert_eq!(
        inbox_row.accesskit_node().toggled(),
        Some(egui::accesskit::Toggled::True),
        "the open mailbox is pressed"
    );
}

#[test]
fn high_contrast_themes_and_text_scale() {
    let mut h = gui(synced_state());
    h.run();
    let theme_id = |id: &str| app_theme::builtin::get(id).unwrap().content;
    for (value, dark, surface) in [
        (3_u64, true, theme_id("dail-dark-hc")),
        (4, false, theme_id("dail-light-hc")),
        // 3 and 4 named the high-contrast themes (UI-11): dark mode keeps `dail-dark-hc`.
        (2, true, theme_id("dail-dark-hc")),
    ] {
        apply(
            &mut h,
            Command::SetSetting {
                key: SettingKey::Theme,
                value: value.into(),
            },
        );
        h.run();
        assert_eq!(
            h.ctx.global_style().visuals.dark_mode,
            dark,
            "theme {value}"
        );
        let t = theme::tokens(&h.ctx);
        assert_eq!(
            (t.text_strong, t.border),
            (
                theme::color(surface.text_strong),
                theme::color(surface.border)
            ),
            "theme {value}"
        );
    }
    apply(
        &mut h,
        Command::SetSetting {
            key: SettingKey::TextScale,
            value: 150.into(),
        },
    );
    h.run();
    assert!((h.ctx.zoom_factor() - 1.5).abs() < 0.001);
}

#[test]
fn tab_walks_the_wizard_fields_in_reading_order() {
    let mut h = gui(AppState::default());
    h.run();
    h.get_by_label("Other IMAP / SMTP").click();
    h.run();
    h.get_by_label("Next").click();
    h.run();
    h.get_by_label("Email address").click();
    h.run();
    for next in ["Name (optional)", "IMAP server", "SMTP server"] {
        h.key_press(Key::Tab);
        h.run();
        assert!(h.get_by_label(next).is_focused(), "Tab moves to {next}");
    }
    assert!(
        h.state_mut().pump().is_empty(),
        "Tab in a form is not focus_pane"
    );
}

#[test]
fn an_error_toast_carries_the_error_icon() {
    let mut h = gui(synced_state());
    h.run();
    // A rejected command: a thread that does not exist.
    let id: app_core::ThreadId =
        serde_json::from_value("00000000-0000-0000-0000-00000000beef".into()).unwrap();
    h.state_mut()
        .out
        .push(app_gui::Request::user(Command::OpenThread { id }));
    h.state_mut().pump();
    h.run();
    h.get(By::new().role(Role::Image).label("Error"));
}
