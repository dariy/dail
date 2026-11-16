//! The icons of `app_front::actions` (PAR-1) are `app_theme` icons.

use app_theme::Icon;

#[test]
fn every_action_icon_is_a_theme_icon() {
    for name in app_front::actions::ICONS {
        assert!(
            Icon::ALL.iter().any(|icon| icon.name() == *name),
            "{name} is not an app_theme::Icon"
        );
    }
}
