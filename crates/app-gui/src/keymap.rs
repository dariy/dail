//! The GUI's key handling: egui events mapped onto the shared keymap (`app_front::keymap`).
//!
//! Bindings, sequences and what a key does in the view are toolkit-neutral; this module only
//! turns egui's key and text events into [`Chord`]s, feeds them in, and consumes the events the
//! keymap used.

use app_core::AppState;
use app_front::keymap::{Chord, Key, Mods, Press};
use egui::Event;

pub use app_front::keymap::{Action, Binding, bindings, parse_chord, parse_sequence, resolve};

/// The neutral key of an egui key, or the character of a symbol key (`/`, `-`).
enum Mapped {
    Key(Key),
    Symbol(char),
}

fn map_key(key: egui::Key) -> Option<Mapped> {
    use egui::Key as E;
    let key = match key {
        E::Escape => Key::Escape,
        E::Enter => Key::Enter,
        E::Tab => Key::Tab,
        E::Backspace => Key::Backspace,
        E::Delete => Key::Delete,
        E::Space => Key::Space,
        E::ArrowUp => Key::ArrowUp,
        E::ArrowDown => Key::ArrowDown,
        E::ArrowLeft => Key::ArrowLeft,
        E::ArrowRight => Key::ArrowRight,
        E::PageUp => Key::PageUp,
        E::PageDown => Key::PageDown,
        E::Home => Key::Home,
        E::End => Key::End,
        other => {
            let name = other.symbol_or_name();
            if let Some(n) = name.strip_prefix('F')
                && let Ok(n) = n.parse::<u8>()
            {
                return Some(Mapped::Key(Key::F(n)));
            }
            let mut chars = name.chars();
            return match (chars.next(), chars.next()) {
                (Some(c), None) if c.is_ascii_alphabetic() => {
                    Some(Mapped::Key(Key::Letter(c.to_ascii_lowercase())))
                }
                (Some(c), None) if c.is_ascii_digit() => Some(Mapped::Key(Key::Digit(c))),
                (Some(c), None) if !c.is_whitespace() => Some(Mapped::Symbol(c)),
                _ => None,
            };
        }
    };
    Some(Mapped::Key(key))
}

const fn mods(m: egui::Modifiers) -> Mods {
    Mods {
        ctrl: m.command,
        shift: m.shift,
        alt: m.alt,
    }
}

/// The chord of each event that has one, by event index. A symbol arrives both as a key event
/// (`Slash`) and as text (`/`), or only as text (`#`); either way it is one [`Chord::Text`].
fn chords(events: &[Event]) -> Vec<(Vec<usize>, Chord)> {
    // (event indices, chord, from a key event)
    let mut out: Vec<(Vec<usize>, Chord, bool)> = Vec::new();
    for (n, event) in events.iter().enumerate() {
        match event {
            Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => match map_key(*key) {
                Some(Mapped::Key(k)) => {
                    out.push((vec![n], Chord::Key(mods(*modifiers), k), true));
                }
                Some(Mapped::Symbol(c)) if !modifiers.command && !modifiers.alt => {
                    out.push((vec![n], Chord::Text(c), true));
                }
                _ => {}
            },
            // Letters and digits arrive as key events too; only symbols are matched as text.
            Event::Text(text) => {
                let mut chars = text.chars();
                let (Some(c), None) = (chars.next(), chars.next()) else {
                    continue;
                };
                if c.is_ascii_alphanumeric() || c.is_whitespace() {
                    continue;
                }
                if let Some((indices, _, _)) = out.iter_mut().find(|(indices, chord, key)| {
                    *key && *chord == Chord::Text(c) && indices.len() == 1
                }) {
                    // The text of a symbol key already seen this frame.
                    indices.push(n);
                } else {
                    out.push((vec![n], Chord::Text(c), false));
                }
            }
            _ => {}
        }
    }
    out.into_iter()
        .map(|(indices, chord, _)| (indices, chord))
        .collect()
}

/// UI-local key state: the shared bindings and sequence state.
#[derive(Debug, Default)]
pub struct KeyState {
    inner: app_front::keymap::KeyState,
}

impl KeyState {
    /// The bindings, in registry order.
    pub fn bindings(&self) -> &[Binding] {
        self.inner.bindings()
    }

    /// Reads this frame's key presses and returns the actions of the bindings they complete.
    /// Matched presses are consumed.
    pub fn handle(&mut self, ctx: &egui::Context, state: &AppState) -> Vec<Action> {
        let typing = ctx.egui_wants_keyboard_input();
        // egui's time is in seconds; the sequence timeout is counted in milliseconds.
        let now_ms = ctx.input(|i| (i.time * 1000.0) as i64);
        let pressed = ctx.input(|i| chords(&i.events));
        let mut actions = Vec::new();
        let mut consumed: Vec<usize> = Vec::new();
        for (indices, chord) in pressed {
            match self.inner.press(chord, typing, state, now_ms) {
                Press::Done(action) => actions.push(action),
                Press::Pending => {}
                Press::Unbound => continue,
            }
            consumed.extend(indices);
        }
        if !consumed.is_empty() {
            // Tab and the arrows also move egui's widget focus; a key the keymap used must not.
            let navigates = ctx.input(|i| {
                consumed.iter().any(|n| {
                    matches!(
                        i.events.get(*n),
                        Some(Event::Key {
                            key: egui::Key::Tab
                                | egui::Key::ArrowUp
                                | egui::Key::ArrowDown
                                | egui::Key::ArrowLeft
                                | egui::Key::ArrowRight,
                            ..
                        })
                    )
                })
            });
            if navigates {
                ctx.memory_mut(|m| m.move_focus(egui::FocusDirection::None));
            }
            ctx.input_mut(|i| {
                let mut n = 0;
                i.events.retain(|_| {
                    let keep = !consumed.contains(&n);
                    n += 1;
                    keep
                });
            });
        }
        actions
    }
}
