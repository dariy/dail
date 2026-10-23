//! Hardware keys: Android's key events as the shared keymap's chords (`app_front::keymap`), and
//! which chords Android hands to an app at all, so the desktop's keys work on tablets and
//! Chromebooks.
//!
//! Kotlin sends what Android reports for a key that no text field took (`UiEvent::KeyDown`):
//! the key code, the character with Shift only, and the modifiers. The mapping is here, not in
//! Kotlin. Android and ChromeOS keep some chords for themselves: Meta (the Search or Windows key)
//! with anything, Ctrl+Space and Ctrl+Shift+Space (the next keyboard layout), Alt+Tab (the app
//! switcher), and the function keys (on a Chromebook the top row is brightness and volume unless
//! Search is held).

use app_core::{CommandName, Settings};
use app_front::keymap::{Chord, Key, Mods, parse_sequence};

/// Android's key codes (`android.view.KeyEvent.KEYCODE_*`) that the keymap names.
mod code {
    pub const DIGIT_0: i32 = 7;
    pub const DIGIT_9: i32 = 16;
    pub const DPAD_UP: i32 = 19;
    pub const DPAD_DOWN: i32 = 20;
    pub const DPAD_LEFT: i32 = 21;
    pub const DPAD_RIGHT: i32 = 22;
    pub const A: i32 = 29;
    pub const Z: i32 = 54;
    pub const TAB: i32 = 61;
    pub const SPACE: i32 = 62;
    pub const ENTER: i32 = 66;
    pub const DEL: i32 = 67;
    pub const PAGE_UP: i32 = 92;
    pub const PAGE_DOWN: i32 = 93;
    pub const ESCAPE: i32 = 111;
    pub const FORWARD_DEL: i32 = 112;
    pub const MOVE_HOME: i32 = 122;
    pub const MOVE_END: i32 = 123;
    pub const F1: i32 = 131;
    pub const F12: i32 = 142;
    pub const NUMPAD_ENTER: i32 = 160;
}

/// What Android reports for one key press.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AndroidKey {
    /// `KeyEvent.getKeyCode()`.
    pub code: i32,
    /// `KeyEvent.getUnicodeChar(meta without Ctrl, Alt and Meta)`: the character as typed with
    /// Shift only; 0 for none.
    pub unicode: u32,
    /// Ctrl is held.
    pub ctrl: bool,
    /// Shift is held.
    pub shift: bool,
    /// Alt is held.
    pub alt: bool,
    /// Meta (Search, Windows, Cmd) is held.
    pub meta: bool,
}

/// The chord of a key press, or `None` for keys the keymap has no name for (a modifier alone)
/// and for chords with Meta, which belong to the system.
pub fn chord_of(key: AndroidKey) -> Option<Chord> {
    if key.meta {
        return None;
    }
    let mods = Mods {
        ctrl: key.ctrl,
        shift: key.shift,
        alt: key.alt,
    };
    let named = match key.code {
        code::ENTER | code::NUMPAD_ENTER => Some(Key::Enter),
        code::TAB => Some(Key::Tab),
        code::DEL => Some(Key::Backspace),
        code::FORWARD_DEL => Some(Key::Delete),
        code::ESCAPE => Some(Key::Escape),
        code::DPAD_UP => Some(Key::ArrowUp),
        code::DPAD_DOWN => Some(Key::ArrowDown),
        code::DPAD_LEFT => Some(Key::ArrowLeft),
        code::DPAD_RIGHT => Some(Key::ArrowRight),
        code::PAGE_UP => Some(Key::PageUp),
        code::PAGE_DOWN => Some(Key::PageDown),
        code::MOVE_HOME => Some(Key::Home),
        code::MOVE_END => Some(Key::End),
        code::SPACE => Some(Key::Space),
        code @ code::F1..=code::F12 => u8::try_from(code - code::F1 + 1).ok().map(Key::F),
        _ => None,
    };
    if let Some(named) = named {
        return Some(Chord::Key(mods, named));
    }
    let held = key.ctrl || key.alt;
    if held {
        let key = match key.code {
            code @ code::A..=code::Z => {
                Key::Letter(char::from(b'a' + u8::try_from(code - code::A).ok()?))
            }
            code @ code::DIGIT_0..=code::DIGIT_9 => {
                Key::Digit(char::from(b'0' + u8::try_from(code - code::DIGIT_0).ok()?))
            }
            _ => {
                let c = char::from_u32(key.unicode)?;
                if !c.is_ascii_punctuation() {
                    return None;
                }
                // Shift is part of the symbol (`Ctrl+?`).
                return Some(Chord::Key(
                    Mods {
                        shift: false,
                        ..mods
                    },
                    Key::Symbol(c),
                ));
            }
        };
        return Some(Chord::Key(mods, key));
    }
    // A typed character: Shift is part of a symbol, and of an upper-case letter.
    Chord::typed(char::from_u32(key.unicode)?)
}

/// Whether Android delivers `chord` to an app.
pub const fn delivers(chord: Chord) -> bool {
    match chord {
        Chord::Text(_) => true,
        Chord::Key(m, key) => match key {
            Key::F(_) => false,
            Key::Space => !m.ctrl,
            Key::Tab => !m.alt,
            Key::Letter(_)
            | Key::Digit(_)
            | Key::Symbol(_)
            | Key::Escape
            | Key::Enter
            | Key::Backspace
            | Key::Delete
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

/// The keys of `name` in `settings` that Android delivers, for the settings and the palette.
pub fn deliverable(settings: &Settings, name: CommandName) -> Vec<String> {
    app_front::keymap::keys_for(settings, name)
        .into_iter()
        .filter(|text| parse_sequence(text).is_some_and(|keys| keys.into_iter().all(delivers)))
        .collect()
}
