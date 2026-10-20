//! Terminal keys: crossterm's key events as the shared keymap's chords (`app_front::keymap`), and
//! which chords a plain terminal delivers at all.
//!
//! The TUI assumes a plain xterm-like terminal (Termius, Blink, tmux, mosh): Ctrl+Enter arrives as
//! Enter and Ctrl+Shift+letter as Ctrl+letter, F-keys and Esc may be missing on an iPad keyboard,
//! Ctrl+B and Ctrl+A belong to tmux and screen, and there is no kitty keyboard protocol.

use app_front::keymap::{Chord, Key, Mods};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// The chord of a key event, or `None` for keys the keymap has no name for.
pub fn chord_of(event: &KeyEvent) -> Option<Chord> {
    let mods = Mods {
        ctrl: event.modifiers.contains(KeyModifiers::CONTROL),
        shift: event.modifiers.contains(KeyModifiers::SHIFT),
        alt: event.modifiers.contains(KeyModifiers::ALT),
    };
    let key = match event.code {
        KeyCode::Char(c) if mods.ctrl || mods.alt => {
            if c.is_ascii_alphabetic() {
                Key::Letter(c.to_ascii_lowercase())
            } else if c.is_ascii_digit() {
                Key::Digit(c)
            } else {
                return None;
            }
        }
        // A typed character: Shift is part of a symbol, and of an upper-case letter.
        KeyCode::Char(c) => return Chord::typed(c),
        KeyCode::Enter => Key::Enter,
        KeyCode::Tab => Key::Tab,
        KeyCode::BackTab => {
            return Some(Chord::Key(
                Mods {
                    shift: true,
                    ..mods
                },
                Key::Tab,
            ));
        }
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Delete => Key::Delete,
        KeyCode::Esc => Key::Escape,
        KeyCode::Up => Key::ArrowUp,
        KeyCode::Down => Key::ArrowDown,
        KeyCode::Left => Key::ArrowLeft,
        KeyCode::Right => Key::ArrowRight,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::F(n) => Key::F(n),
        _ => return None,
    };
    let mods = match key {
        // A letter's Shift was folded into `Chord::typed`; with Ctrl it can't be told apart.
        Key::Letter(_) if mods.ctrl => Mods {
            shift: false,
            ..mods
        },
        _ => mods,
    };
    Some(Chord::Key(mods, key))
}

/// Whether a plain terminal delivers `chord` as itself.
pub const fn delivers(chord: Chord) -> bool {
    match chord {
        Chord::Text(_) => true,
        Chord::Key(m, key) => match key {
            Key::F(_) => false,
            // tmux and screen take Ctrl+B and Ctrl+A; Ctrl+I, Ctrl+M and Ctrl+H are Tab, Enter
            // and Backspace; Ctrl+Shift+letter arrives as Ctrl+letter.
            Key::Letter(c) => !(m.ctrl && (m.shift || matches!(c, 'a' | 'b' | 'h' | 'i' | 'm'))),
            Key::Digit(_) | Key::Space => !m.ctrl,
            // Ctrl+Enter arrives as Enter, Ctrl+Tab as Tab.
            Key::Enter | Key::Tab | Key::Backspace | Key::Escape => !m.ctrl && !m.alt,
            Key::Delete
            | Key::ArrowUp
            | Key::ArrowDown
            | Key::ArrowLeft
            | Key::ArrowRight
            | Key::PageUp
            | Key::PageDown
            | Key::Home
            | Key::End => true,
        },
    }
}

/// The keys of `name` in `settings` that a plain terminal delivers, for hints and help.
pub fn deliverable(settings: &app_core::Settings, name: app_core::CommandName) -> Vec<String> {
    app_front::keymap::keys_for(settings, name)
        .into_iter()
        .filter(|text| {
            app_front::keymap::parse_sequence(text)
                .is_some_and(|keys| keys.into_iter().all(delivers))
        })
        .collect()
}

/// The first key of `name` that a plain terminal delivers, for hints.
pub fn hint(settings: &app_core::Settings, name: app_core::CommandName) -> Option<String> {
    deliverable(settings, name).into_iter().next()
}

#[cfg(test)]
mod tests {
    use app_front::keymap::parse_chord;

    use super::*;

    fn event(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn terminal_events_map_onto_the_registry_chords() {
        let none = KeyModifiers::NONE;
        assert_eq!(chord_of(&event(KeyCode::Char('u'), none)), parse_chord("u"));
        assert_eq!(
            chord_of(&event(KeyCode::Char('U'), KeyModifiers::SHIFT)),
            parse_chord("Shift+U")
        );
        assert_eq!(
            chord_of(&event(KeyCode::Char('#'), KeyModifiers::SHIFT)),
            parse_chord("#")
        );
        assert_eq!(chord_of(&event(KeyCode::Char(':'), none)), parse_chord(":"));
        assert_eq!(
            chord_of(&event(KeyCode::Char('s'), KeyModifiers::CONTROL)),
            parse_chord("Ctrl+S")
        );
        assert_eq!(
            chord_of(&event(
                KeyCode::Char('Z'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT
            )),
            parse_chord("Ctrl+Z"),
            "Ctrl+Shift+Z arrives as Ctrl+Z"
        );
        assert_eq!(
            chord_of(&event(KeyCode::BackTab, KeyModifiers::SHIFT)),
            parse_chord("Shift+Tab")
        );
        assert_eq!(chord_of(&event(KeyCode::Esc, none)), parse_chord("Escape"));
        assert!(!delivers(parse_chord("Ctrl+Enter").unwrap()));
        assert!(!delivers(parse_chord("Ctrl+B").unwrap()));
        assert!(!delivers(parse_chord("F5").unwrap()));
        assert!(delivers(parse_chord("Ctrl+K").unwrap()));
    }
}
