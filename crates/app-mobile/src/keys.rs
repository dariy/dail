//! Hardware keys: which of the keymap's chords Android delivers to an app.

use app_core::{CommandName, Settings};

/// The keys of `name` in `settings` that Android delivers, for the settings and the palette.
pub fn deliverable(settings: &Settings, name: CommandName) -> Vec<String> {
    app_front::keymap::keys_for(settings, name)
}
