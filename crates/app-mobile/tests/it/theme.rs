//! The theme in the frame and the About screen.

use app_core::{Command, ThemeMode};
use app_mobile::UiEvent;

use crate::support::Phone;

#[test]
fn the_theme_comes_once_and_again_when_it_changes() {
    let mut phone = Phone::inbox(360, 800);
    let first = phone
        .frame()
        .theme
        .expect("the first frame carries the theme");
    assert_eq!(first.id, "dail-light");
    assert!(!first.dark);
    assert_eq!(first.content["text"].len(), 9, "#RRGGBBAA");
    assert!(first.content.contains_key("inverse_bg"));
    assert_eq!(first.type_scale["body_md"].face, "sans");
    assert_eq!(first.icons["archive"].chars().count(), 1);
    assert!(phone.frame().theme.is_none(), "unchanged: not sent again");

    // The system turns dark: `theme` 0 follows it.
    phone.event(UiEvent::Configure {
        width_dp: 360,
        height_dp: 800,
        dark: true,
        font_scale: 1.0,
        locale: None,
        keyboard: false,
    });
    let dark = phone.frame().theme.unwrap();
    assert_eq!(dark.id, "dail-dark");
    assert!(dark.dark);

    // The high-contrast dark theme, chosen in the settings.
    phone.dispatch(Command::SetTheme {
        mode: None,
        light: None,
        dark: Some("dail-dark-hc".to_owned()),
    });
    assert_eq!(phone.frame().theme.unwrap().id, "dail-dark-hc");

    // `theme` 1 is light whatever the system says.
    phone.dispatch(Command::SetTheme {
        mode: Some(ThemeMode::Light),
        light: None,
        dark: None,
    });
    assert_eq!(phone.frame().theme.unwrap().id, "dail-light");
}

#[test]
fn about_shows_the_licences_and_back_closes_it() {
    let mut phone = Phone::inbox(360, 800);
    phone.dispatch(Command::FocusPane {
        pane: app_core::Pane::Sidebar,
    });
    let frame = phone.frame();
    assert!(frame.about.is_none());
    assert!(
        frame.top_bar.actions.iter().all(|b| b.label != "About"),
        "MISC-3: About is in the drawer now"
    );
    let footer = frame.drawer.unwrap().footer;
    let entry = footer.iter().find(|i| i.label == "About").unwrap();
    assert_eq!(entry.icon, "info");
    let about = entry.action;
    assert!(
        phone.tap(about).is_empty(),
        "no command: the screen is the presenter's"
    );
    let frame = phone.frame();
    assert!(frame.back);
    assert!(frame.fab.is_none(), "nothing floats over the licences");
    let about = frame.about.unwrap();
    assert!(about.text.contains(env!("CARGO_PKG_VERSION")));
    let licences = about.licences;
    let names: Vec<&str> = licences.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Geist",
            "JetBrains Mono",
            "Material Symbols",
            "Rust crates",
            "Android libraries"
        ]
    );
    assert!(licences[0].text.contains("SIL OPEN FONT LICENSE"));
    // What `libdail.so` links, and nothing that only builds or tests it.
    let crates = &licences[3].text;
    assert!(crates.lines().any(|line| line.starts_with("jni ")));
    assert!(crates.lines().any(|line| line.starts_with("rustls ")));
    assert!(!crates.lines().any(|line| line.starts_with("insta ")));
    assert!(!crates.lines().any(|line| line.starts_with("serde_derive ")));
    assert!(
        !crates.contains("app-"),
        "the workspace's own crates are dail"
    );
    assert!(
        licences[4]
            .text
            .contains("androidx.compose.material3:material3 ")
    );
    phone.take();
    phone.event(UiEvent::Back);
    assert!(phone.take().is_empty());
    assert!(phone.frame().about.is_none());
}

#[test]
fn the_mailboxes_button_is_a_menu_icon_named_mailboxes() {
    // MISC-2: a phone shows the list alone; its navigation button opens the drawer.
    let mut phone = Phone::inbox(360, 800);
    let frame = phone.frame();
    let navigation = frame.top_bar.navigation.unwrap();
    assert_eq!(navigation.icon.as_deref(), Some("menu"));
    assert_eq!(navigation.label, "Mailboxes", "TalkBack's name");
    let icons = frame.theme.unwrap().icons;
    assert!(
        icons.contains_key("menu"),
        "Android draws it from the theme's glyphs"
    );
}

#[test]
fn the_about_screen_has_a_close_button_in_its_top_bar() {
    let mut phone = Phone::inbox(360, 800);
    phone.dispatch(Command::FocusPane {
        pane: app_core::Pane::Sidebar,
    });
    let footer = phone.frame().drawer.unwrap().footer;
    let about = footer.iter().find(|i| i.label == "About").unwrap().action;
    phone.tap(about);
    let frame = phone.frame();
    assert!(frame.about.is_some());
    assert!(frame.drawer.is_none(), "the modal drawer gives way");
    assert_eq!(frame.top_bar.title, "About");
    let close = frame.top_bar.navigation.unwrap();
    assert_eq!(
        (close.icon.as_deref(), close.label.as_str()),
        (Some("close"), "Close")
    );
    assert!(phone.tap(close.action).is_empty());
    assert!(phone.frame().about.is_none(), "closed");
}
