//! Hardware keyboards: Android's key events as the registry's chords, every bound command
//! reachable by a key Android delivers, and a key press driving the presenter.

use app_core::{Command, CommandName};
use app_front::keymap::{parse_chord, parse_sequence};
use app_mobile::UiEvent;
use app_mobile::keys::{AndroidKey, chord_of, deliverable, delivers};

use crate::support::{Phone, inbox_of};

/// A key press: `code` with the character `c` (0 for none).
fn key(code: i32, c: char) -> AndroidKey {
    AndroidKey {
        code,
        unicode: u32::from(c),
        ..AndroidKey::default()
    }
}

fn ctrl(k: AndroidKey) -> AndroidKey {
    AndroidKey { ctrl: true, ..k }
}

fn shift(k: AndroidKey) -> AndroidKey {
    AndroidKey { shift: true, ..k }
}

#[test]
fn android_key_events_map_onto_the_registry_chords() {
    // KEYCODE_E = 33, KEYCODE_Z = 54, KEYCODE_K = 39, KEYCODE_3 = 10, KEYCODE_COMMA = 55.
    assert_eq!(chord_of(key(33, 'e')), parse_chord("e"));
    assert_eq!(chord_of(shift(key(33, 'E'))), parse_chord("Shift+E"));
    assert_eq!(chord_of(shift(key(10, '#'))), parse_chord("#"));
    assert_eq!(chord_of(key(76, '/')), parse_chord("/"));
    assert_eq!(chord_of(ctrl(key(39, '\0'))), parse_chord("Ctrl+K"));
    assert_eq!(
        chord_of(shift(ctrl(key(54, 'Z')))),
        parse_chord("Ctrl+Shift+Z")
    );
    assert_eq!(chord_of(ctrl(key(55, ','))), parse_chord("Ctrl+,"));
    assert_eq!(chord_of(ctrl(key(10, '3'))), parse_chord("Ctrl+3"));
    assert_eq!(chord_of(key(111, '\0')), parse_chord("Escape"));
    assert_eq!(chord_of(ctrl(key(66, '\n'))), parse_chord("Ctrl+Enter"));
    assert_eq!(chord_of(key(160, '\n')), parse_chord("Enter"));
    assert_eq!(chord_of(shift(key(61, '\t'))), parse_chord("Shift+Tab"));
    assert_eq!(chord_of(key(20, '\0')), parse_chord("ArrowDown"));
    assert_eq!(chord_of(key(62, ' ')), parse_chord("Space"));
    assert_eq!(chord_of(key(131, '\0')), parse_chord("F1"));
    // A modifier alone (KEYCODE_SHIFT_LEFT = 59) and anything with Meta are not the app's.
    assert_eq!(chord_of(shift(key(59, '\0'))), None);
    assert_eq!(
        chord_of(AndroidKey {
            meta: true,
            ..key(33, 'e')
        }),
        None
    );
}

#[test]
fn every_command_with_default_keys_has_one_android_delivers() {
    let gmail = app_core::Settings::default();
    let vim = app_core::Settings {
        key_preset: app_core::KeyPreset::Vim,
        ..app_core::Settings::default()
    };
    for &name in CommandName::ALL {
        let keys = name.spec().default_keys;
        if keys.is_empty() {
            continue;
        }
        let deliverable_here = keys.iter().any(|text| {
            parse_sequence(text).is_some_and(|chords| chords.into_iter().all(delivers))
        });
        assert!(
            deliverable_here,
            "{name:?}: none of {keys:?} reaches Android"
        );
        assert!(!deliverable(&gmail, name).is_empty(), "{name:?}");
        assert!(
            !deliverable(&vim, name).is_empty(),
            "{name:?} with the Vim keys"
        );
    }
    // What the system keeps.
    for text in ["Ctrl+Space", "Alt+Tab", "F5"] {
        assert!(!delivers(parse_chord(text).unwrap()), "{text}");
    }
}

#[test]
fn every_key_a_bound_command_has_reaches_it_from_android_events() {
    // Each deliverable chord of the bindings can be produced by some Android key event: the
    // mapping covers every key the keymap names but the function keys.
    let settings = app_core::Settings::default();
    for &name in CommandName::ALL {
        for text in deliverable(&settings, name) {
            for chord in parse_sequence(&text).unwrap() {
                assert!(produced(chord), "{name:?}: {text} has no Android key event");
            }
        }
    }
}

/// Whether some key event maps onto `chord` (a search over the codes, the printable ASCII
/// characters and the modifiers).
fn produced(chord: app_front::keymap::Chord) -> bool {
    let chars = std::iter::once('\0').chain((0x20u8..0x7f).map(char::from));
    let chars: Vec<char> = chars.collect();
    for code in 0..=170 {
        for &c in &chars {
            for bits in 0..8u8 {
                let k = AndroidKey {
                    code,
                    unicode: u32::from(c),
                    ctrl: bits & 1 != 0,
                    shift: bits & 2 != 0,
                    alt: bits & 4 != 0,
                    meta: false,
                };
                if chord_of(k) == Some(chord) {
                    return true;
                }
            }
        }
    }
    false
}

#[test]
fn a_hardware_key_drives_the_presenter_like_the_desktop() {
    let mut phone = Phone::inbox(1280, 800);
    let (_, threads) = inbox_of(phone.state());
    phone.dispatch(Command::OpenThread { id: threads[0] });
    phone.take();
    // `e` archives the open thread, as on the desktop (KEYCODE_E = 33).
    phone.event(UiEvent::KeyDown {
        code: 33,
        unicode: u32::from('e'),
        ctrl: false,
        shift: false,
        alt: false,
        meta: false,
    });
    let got = crate::support::requests_of(&phone.take());
    assert!(
        matches!(got.first(), Some(Command::ArchiveThreads { ids, .. }) if ids == &[threads[0]]),
        "{got:?}"
    );
    // Ctrl+K opens the palette (KEYCODE_K = 39).
    phone.event(UiEvent::KeyDown {
        code: 39,
        unicode: u32::from('k'),
        ctrl: true,
        shift: false,
        alt: false,
        meta: false,
    });
    assert_eq!(
        crate::support::requests_of(&phone.take()),
        [Command::OpenPalette {}]
    );
    // The palette's rows hint only keys Android delivers.
    let sheet = phone.frame().palette.unwrap();
    for row in sheet.sections.iter().flat_map(|s| &s.rows) {
        if let Some(keys) = &row.keys {
            let chords = parse_sequence(keys).unwrap();
            assert!(chords.into_iter().all(delivers), "{}: {keys}", row.title);
        }
    }
}
