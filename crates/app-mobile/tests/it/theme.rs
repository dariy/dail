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
    let frame = phone.frame();
    assert!(frame.about.is_none());
    let about = frame
        .top_bar
        .actions
        .iter()
        .find(|b| b.label == "About")
        .unwrap()
        .action;
    assert!(
        phone.tap(about).is_empty(),
        "no command: the screen is the presenter's"
    );
    let frame = phone.frame();
    assert!(frame.back);
    assert!(frame.fab.is_none(), "nothing floats over the licences");
    let licences = frame.about.unwrap().licences;
    let names: Vec<&str> = licences.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["Geist", "JetBrains Mono", "Material Symbols"]);
    assert!(licences[0].text.contains("SIL OPEN FONT LICENSE"));
    phone.take();
    phone.event(UiEvent::Back);
    assert!(phone.take().is_empty());
    assert!(phone.frame().about.is_none());
}
