//! The keymap: `CommandSpec.default_keys` (`"Ctrl+K"`, `"Shift+U"`, `"#"`, `"g i"`) turned into
//! toolkit-neutral bindings, the sequence state, and what a binding does in the current view.
//!
//! A front-end maps its key events onto [`Chord`]s and feeds them to [`KeyState::press`]. A key
//! can be bound to several commands (`Escape`, `u`); the first one in registry order that applies
//! wins. Arguments come from the view like in the palette ([`palette::flow`]); a command that
//! still needs one opens the palette at that step (`v` → *Move to…*). While a text field has the
//! focus only `Escape` and Ctrl chords that are not text editing keys act.

use std::collections::BTreeMap;

use app_core::{AppState, Command, CommandName, KeyPreset, Pane, Settings, ThreadId, listing};

use crate::palette::{self, GO_TO};

/// Modifier keys, matched exactly.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Mods {
    /// Ctrl (Cmd on a Mac keyboard in the GUI).
    pub ctrl: bool,
    /// Shift.
    pub shift: bool,
    /// Alt.
    pub alt: bool,
}

impl Mods {
    /// No modifier.
    pub const NONE: Self = Self {
        ctrl: false,
        shift: false,
        alt: false,
    };
    /// Ctrl alone.
    pub const CTRL: Self = Self {
        ctrl: true,
        shift: false,
        alt: false,
    };
    /// Shift alone.
    pub const SHIFT: Self = Self {
        ctrl: false,
        shift: true,
        alt: false,
    };

    /// Whether no modifier is held.
    pub const fn is_none(self) -> bool {
        !self.ctrl && !self.shift && !self.alt
    }
}

/// A key that does not depend on the keyboard layout's symbols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// A letter, lower case (`Shift+U` is `Shift` + `u`).
    Letter(char),
    /// A digit.
    Digit(char),
    /// Escape.
    Escape,
    /// Enter / Return.
    Enter,
    /// Tab.
    Tab,
    /// Backspace.
    Backspace,
    /// Delete.
    Delete,
    /// Space.
    Space,
    /// ↑.
    ArrowUp,
    /// ↓.
    ArrowDown,
    /// ←.
    ArrowLeft,
    /// →.
    ArrowRight,
    /// Page Up.
    PageUp,
    /// Page Down.
    PageDown,
    /// Home.
    Home,
    /// End.
    End,
    /// A function key, `F1`–`F24`.
    F(u8),
    /// A symbol key pressed with Ctrl or Alt (`Ctrl+,`). Without them a symbol is
    /// [`Chord::Text`], as typed.
    Symbol(char),
}

impl Key {
    /// Parses a key name as written in `default_keys`: `Escape`, `Enter`, `ArrowUp`, `F5`, `k`,
    /// `K`, `3`.
    pub fn from_name(name: &str) -> Option<Self> {
        let named = match name {
            "Escape" | "Esc" => Self::Escape,
            "Enter" | "Return" => Self::Enter,
            "Tab" => Self::Tab,
            "Backspace" => Self::Backspace,
            "Delete" => Self::Delete,
            "Space" => Self::Space,
            "ArrowUp" | "Up" => Self::ArrowUp,
            "ArrowDown" | "Down" => Self::ArrowDown,
            "ArrowLeft" | "Left" => Self::ArrowLeft,
            "ArrowRight" | "Right" => Self::ArrowRight,
            "PageUp" => Self::PageUp,
            "PageDown" => Self::PageDown,
            "Home" => Self::Home,
            "End" => Self::End,
            other => {
                if let Some(n) = other.strip_prefix('F')
                    && let Ok(n) = n.parse::<u8>()
                    && (1..=24).contains(&n)
                {
                    return Some(Self::F(n));
                }
                let mut chars = other.chars();
                return match (chars.next(), chars.next()) {
                    (Some(c), None) if c.is_ascii_alphabetic() => {
                        Some(Self::Letter(c.to_ascii_lowercase()))
                    }
                    (Some(c), None) if c.is_ascii_digit() => Some(Self::Digit(c)),
                    _ => None,
                };
            }
        };
        Some(named)
    }
}

/// One key with modifiers, or a typed symbol (`#`, `/`, `:`) whose key depends on the layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Chord {
    /// A key, matched with exactly these modifiers (Shift included).
    Key(Mods, Key),
    /// A symbol as typed (Shift is part of the character).
    Text(char),
}

impl Chord {
    /// A key without modifiers.
    pub const fn plain(key: Key) -> Self {
        Self::Key(Mods::NONE, key)
    }

    /// The chord for a typed character: a letter or digit becomes its key (an upper-case letter
    /// with Shift), any other printable character a [`Chord::Text`].
    pub fn typed(c: char) -> Option<Self> {
        if c.is_ascii_alphabetic() {
            let mods = if c.is_ascii_uppercase() {
                Mods::SHIFT
            } else {
                Mods::NONE
            };
            Some(Self::Key(mods, Key::Letter(c.to_ascii_lowercase())))
        } else if c.is_ascii_digit() {
            Some(Self::plain(Key::Digit(c)))
        } else if c == ' ' {
            Some(Self::plain(Key::Space))
        } else if c.is_control() || c.is_whitespace() {
            None
        } else {
            Some(Self::Text(c))
        }
    }
}

/// Parses one chord, e.g. `Ctrl+Shift+Z`, `Escape`, `j`, `#`.
pub fn parse_chord(text: &str) -> Option<Chord> {
    let mut mods = Mods::NONE;
    let mut parts: Vec<&str> = text.split('+').collect();
    // A lone `+` would split into two empty parts.
    let last = parts.pop()?;
    for part in parts {
        match part {
            "Ctrl" | "Cmd" => mods.ctrl = true,
            "Shift" => mods.shift = true,
            "Alt" => mods.alt = true,
            _ => return None,
        }
    }
    if let Some(key) = Key::from_name(last) {
        // Single letters are matched as keys so that Shift+U is not the text "U".
        return Some(Chord::Key(mods, key));
    }
    let mut chars = last.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if (mods.ctrl || mods.alt) && c.is_ascii_punctuation() => {
            Some(Chord::Key(mods, Key::Symbol(c)))
        }
        (Some(c), None) if mods.is_none() && !c.is_whitespace() && !c.is_control() => {
            Some(Chord::Text(c))
        }
        _ => None,
    }
}

/// Parses a sequence of chords separated by spaces, e.g. `g i`.
pub fn parse_sequence(text: &str) -> Option<Vec<Chord>> {
    text.split_whitespace().map(parse_chord).collect()
}

/// One binding of the keymap: a key sequence and the command it triggers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// The chords to press, in order.
    pub keys: Vec<Chord>,
    /// The sequence as written in the spec or the settings, e.g. `g i`.
    pub text: String,
    /// The command.
    pub name: CommandName,
}

/// What a key leads to.
#[derive(Debug)]
pub enum Action {
    /// Submit this command.
    Run(Command),
    /// The command needs an argument: open the palette at that step.
    Ask(palette::Ask),
    /// Open the account wizard.
    Wizard,
    /// Open the password form for an account.
    Password(app_core::AccountId),
}

/// The Vim preset's keys, where they differ from the registry's (Gmail's).
fn vim(name: CommandName) -> Option<&'static [&'static str]> {
    Some(match name {
        CommandName::Undo => &["u", "Ctrl+Z"],
        CommandName::Redo => &["Ctrl+R", "Ctrl+Shift+Z"],
        CommandName::SyncNow => &["F5", "g r"],
        CommandName::TrashThreads => &["d d", "#"],
        CommandName::MarkRead => &["m r", "Shift+I"],
        CommandName::MarkUnread => &["m u", "Shift+U"],
        CommandName::CollapseFolders => &["ArrowLeft", "h"],
        CommandName::ExpandFolders => &["ArrowRight", "l"],
        _ => return None,
    })
}

/// The keys of `name` before overrides: the preset's.
fn preset_keys(preset: KeyPreset, name: CommandName) -> Vec<String> {
    let keys = match preset {
        KeyPreset::Vim => vim(name).unwrap_or(name.spec().default_keys),
        KeyPreset::Gmail => name.spec().default_keys,
    };
    keys.iter().map(|k| (*k).to_owned()).collect()
}

/// The keys of `name` in `settings`: its override (`set_keybinding`), else its preset keys
/// without the ones another command's override has taken.
pub fn keys_for(settings: &Settings, name: CommandName) -> Vec<String> {
    if let Some(keys) = settings.keys.get(name.as_str()) {
        return keys.clone();
    }
    preset_keys(settings.key_preset, name)
        .into_iter()
        .filter(|key| !settings.keys.values().flatten().any(|taken| taken == key))
        .collect()
}

/// Every binding of `settings`, in registry order (earlier specs win when several bindings
/// share a key and apply). Unparsable keys are skipped (the registry test checks that the
/// defaults have none).
pub fn bindings(settings: &Settings) -> Vec<Binding> {
    let mut out = Vec::new();
    for &name in CommandName::ALL {
        for text in keys_for(settings, name) {
            if let Some(keys) = parse_sequence(&text) {
                out.push(Binding { keys, text, name });
            }
        }
    }
    out
}

/// What `binding` does in `state`, or `None` if it does not apply (another binding of the same
/// key may).
pub fn resolve(binding: &Binding, state: &AppState) -> Option<Action> {
    let name = binding.name;
    if !(name.spec().available)(state) {
        return None;
    }
    let run = |cmd| Some(Action::Run(cmd));
    if state.view.focus == Pane::Sidebar {
        // In the mailboxes, the list keys move between mailboxes and Enter goes back to the list.
        match name {
            CommandName::SelectThreads => {
                let step: isize = if matches!(binding.text.as_str(), "k" | "ArrowUp") {
                    -1
                } else {
                    1
                };
                return next_mailbox(state, step)
                    .map(|id| Action::Run(Command::OpenMailbox { id }));
            }
            CommandName::OpenThread => {
                return run(Command::FocusPane { pane: Pane::List });
            }
            CommandName::CollapseFolders | CommandName::ExpandFolders => {
                return fold_open_folder(state, name == CommandName::CollapseFolders)
                    .map(Action::Run);
            }
            _ => {}
        }
    }
    match name {
        CommandName::SelectThreads => {
            let step: isize = if matches!(binding.text.as_str(), "k" | "ArrowUp") {
                -1
            } else {
                1
            };
            let next = neighbour(state, step)?;
            if state.view.thread.is_some() {
                run(Command::OpenThread { id: next })
            } else {
                run(Command::SelectThreads { ids: vec![next] })
            }
        }
        CommandName::CheckThreads if binding.text == "x" => {
            // `x` checks the thread under the cursor, or unchecks it.
            let id = state.view.selected.last().copied()?;
            run(if state.view.checked.contains(&id) {
                Command::UncheckThreads { ids: vec![id] }
            } else {
                Command::CheckThreads { ids: vec![id] }
            })
        }
        CommandName::OpenThread => {
            let id = state.view.selected.last().copied()?;
            (state.view.thread != Some(id)).then_some(Action::Run(Command::OpenThread { id }))
        }
        CommandName::OpenMailbox => {
            let letter = binding.text.chars().last()?;
            let role = GO_TO.iter().find(|(c, _)| *c == letter)?.1;
            let account = palette::current_account(state)?;
            let mailbox = state
                .mailboxes
                .values()
                .find(|m| m.account == account && m.role == Some(role))?;
            run(Command::OpenMailbox { id: mailbox.id })
        }
        CommandName::PinThreads => {
            // `Shift+P` toggles: unpin when every targeted thread is pinned (MISC-4).
            let ids = palette::targets(state);
            if ids.is_empty() {
                return None;
            }
            let all = ids
                .iter()
                .filter_map(|t| state.threads.get(t))
                .all(|t| listing::is_pinned(state, t));
            let messages = Vec::new();
            run(if all {
                Command::UnpinThreads { ids, messages }
            } else {
                Command::PinThreads { ids, messages }
            })
        }
        CommandName::MarkRead | CommandName::MarkUnread if binding.text == "u" => {
            // `u` toggles: read if anything targeted is unread, else unread.
            let ids = palette::targets(state);
            let unread = ids.iter().filter_map(|t| state.threads.get(t)).any(|t| {
                t.messages
                    .iter()
                    .filter_map(|m| state.messages.get(m))
                    .any(|m| !m.flags.seen)
            });
            match (name, unread, ids.is_empty()) {
                (_, _, true) => None,
                (CommandName::MarkRead, true, _) => run(Command::MarkRead {
                    ids,
                    messages: Vec::new(),
                }),
                (CommandName::MarkUnread, false, _) => run(Command::MarkUnread {
                    ids,
                    messages: Vec::new(),
                }),
                _ => None,
            }
        }
        CommandName::FocusPane => {
            let step: isize = if binding.text.starts_with("Shift") {
                -1
            } else {
                1
            };
            let panes: Vec<Pane> = Pane::ALL
                .iter()
                .copied()
                .filter(|p| state.view.can_focus(*p))
                .collect();
            let at = panes
                .iter()
                .position(|p| *p == state.view.focus)
                .unwrap_or(0);
            let len = isize::try_from(panes.len()).ok()?;
            let next = usize::try_from((isize::try_from(at).ok()? + step).rem_euclid(len)).ok()?;
            run(Command::FocusPane {
                pane: *panes.get(next)?,
            })
        }
        // The arrows fold folders only in the sidebar.
        CommandName::CollapseFolders | CommandName::ExpandFolders => None,
        // The palette does not offer itself; the key opens it.
        CommandName::OpenPalette => run(Command::OpenPalette {}),
        // Escape closes the innermost thing that is open.
        CommandName::CloseComposer => {
            (state.view.focus == Pane::Composer).then_some(Action::Run(Command::CloseComposer {}))
        }
        CommandName::CloseThread => (state.view.thread.is_some()
            && state.view.focus != Pane::Composer)
            .then_some(Action::Run(Command::CloseThread {})),
        name => match palette::flow(name, state) {
            palette::Flow::Run(cmd) => run(cmd),
            palette::Flow::Ask(ask) => Some(Action::Ask(ask)),
            palette::Flow::Wizard => Some(Action::Wizard),
            palette::Flow::Password(account) => Some(Action::Password(account)),
            palette::Flow::Hidden => None,
        },
    }
}

/// `collapse_folders` / `expand_folders` for the open mailbox's folder, when it has folders
/// below it and is not already so.
fn fold_open_folder(state: &AppState, collapse: bool) -> Option<Command> {
    let key = state.mailboxes.get(&state.view.mailbox?)?.key();
    let nodes = crate::mailbox::tree(state, key.account);
    let node = crate::mailbox::rows(&nodes, &|_| false)
        .into_iter()
        .find(|n| n.key == key)?;
    if !node.is_parent() || state.view.collapsed.contains(&key) == collapse {
        return None;
    }
    let keys = vec![key];
    Some(if collapse {
        Command::CollapseFolders { keys }
    } else {
        Command::ExpandFolders { keys }
    })
}

/// The mailbox `step` rows away from the open one in the sidebar's order (the first one if none
/// is open).
fn next_mailbox(state: &AppState, step: isize) -> Option<app_core::MailboxId> {
    let rows: Vec<app_core::MailboxId> = state
        .accounts
        .keys()
        .flat_map(|account| {
            let nodes = crate::mailbox::sidebar_tree(state, *account);
            crate::mailbox::visible_rows(state, *account, &nodes)
                .iter()
                .filter_map(|node| node.mailbox)
                .collect::<Vec<_>>()
        })
        .collect();
    let Some(at) = state
        .view
        .mailbox
        .and_then(|open| rows.iter().position(|m| *m == open))
    else {
        return rows.first().copied();
    };
    let next = usize::try_from(isize::try_from(at).ok()? + step).ok()?;
    rows.get(next).copied()
}

/// The thread `step` rows away from the cursor in the visible list (the first one if nothing is
/// selected).
fn neighbour(state: &AppState, step: isize) -> Option<ThreadId> {
    let rows = listing::visible_threads(state);
    let cursor = state.view.selected.last().copied().or(state.view.thread);
    let Some(at) = cursor.and_then(|c| rows.iter().position(|r| *r == c)) else {
        return rows.first().copied();
    };
    let next = isize::try_from(at).ok()? + step;
    let next = usize::try_from(next).ok()?;
    rows.get(next).copied()
}

/// Milliseconds within which the next chord of a sequence (`g` … `i`) must follow.
pub const SEQUENCE_TIMEOUT_MS: i64 = 1_500;

/// Letters that keep their text-editing meaning with Ctrl while a text field has the focus.
const EDITING_LETTERS: &[char] = &['a', 'c', 'v', 'x', 'y', 'z'];

/// Whether `binding` acts while a text field has the focus: Escape, and Ctrl chords other than
/// the editing keys (A, C, V, X, Y, Z).
pub fn works_while_typing(binding: &Binding) -> bool {
    match binding.keys.as_slice() {
        [Chord::Key(m, Key::Escape)] => m.is_none(),
        [Chord::Key(m, Key::Letter(c))] => m.ctrl && !EDITING_LETTERS.contains(c),
        [Chord::Key(m, _)] => m.ctrl,
        _ => false,
    }
}

/// What a pressed chord did.
#[derive(Debug)]
pub enum Press {
    /// It completed a binding that applies.
    Done(Action),
    /// It starts (or continues) a longer binding: wait for the next chord.
    Pending,
    /// No binding: the front-end may use the key itself.
    Unbound,
}

/// How far a typed sequence got.
enum Step {
    Done(Action),
    Prefix,
    None,
}

/// The bindings and the chords of a sequence typed so far.
#[derive(Debug)]
pub struct KeyState {
    bindings: Vec<Binding>,
    /// The key settings `bindings` were built from.
    built_for: (KeyPreset, BTreeMap<String, Vec<String>>),
    pending: Vec<Chord>,
    since_ms: i64,
}

impl Default for KeyState {
    fn default() -> Self {
        let settings = Settings::default();
        Self {
            bindings: bindings(&settings),
            built_for: (settings.key_preset, settings.keys),
            pending: Vec::new(),
            since_ms: 0,
        }
    }
}

impl KeyState {
    /// Rebuilds the bindings if the key settings changed.
    pub fn sync(&mut self, settings: &Settings) {
        if self.built_for.0 != settings.key_preset || self.built_for.1 != settings.keys {
            self.bindings = bindings(settings);
            self.built_for = (settings.key_preset, settings.keys.clone());
            self.pending.clear();
        }
    }

    /// The bindings, in registry order.
    pub fn bindings(&self) -> &[Binding] {
        &self.bindings
    }

    /// Whether a sequence is half typed (`g` and waiting).
    pub fn is_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Forgets a half-typed sequence.
    pub fn cancel(&mut self) {
        self.pending.clear();
    }

    fn step(&self, sequence: &[Chord], typing: bool, state: &AppState) -> Step {
        let live = |b: &&Binding| !typing || works_while_typing(b);
        let starts = |b: &Binding| b.keys.iter().zip(sequence).all(|(k, p)| k == p);
        for binding in self
            .bindings
            .iter()
            .filter(live)
            .filter(|b| b.keys.len() == sequence.len() && starts(b))
        {
            if let Some(action) = resolve(binding, state) {
                return Step::Done(action);
            }
        }
        if self
            .bindings
            .iter()
            .filter(live)
            .any(|b| b.keys.len() > sequence.len() && starts(b))
        {
            Step::Prefix
        } else {
            Step::None
        }
    }

    /// Feeds one chord, pressed at `now_ms`. `typing`: a text field has the focus.
    pub fn press(&mut self, chord: Chord, typing: bool, state: &AppState, now_ms: i64) -> Press {
        self.sync(&state.settings);
        if !self.pending.is_empty() && now_ms - self.since_ms > SEQUENCE_TIMEOUT_MS {
            self.pending.clear();
        }
        // A chord that does not continue the pending sequence starts over.
        let mut tries = Vec::with_capacity(2);
        if !self.pending.is_empty() {
            let mut continued = std::mem::take(&mut self.pending);
            continued.push(chord);
            tries.push(continued);
        }
        tries.push(vec![chord]);
        for sequence in tries {
            match self.step(&sequence, typing, state) {
                Step::Done(action) => return Press::Done(action),
                Step::Prefix => {
                    self.pending = sequence;
                    self.since_ms = now_ms;
                    return Press::Pending;
                }
                Step::None => {}
            }
        }
        Press::Unbound
    }

    /// The first key of `name` in `settings`, for hints (`None` if it has none).
    pub fn hint(settings: &Settings, name: CommandName) -> Option<String> {
        keys_for(settings, name).into_iter().next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chords_parse() {
        assert_eq!(
            parse_chord("Ctrl+K"),
            Some(Chord::Key(Mods::CTRL, Key::Letter('k')))
        );
        assert_eq!(
            parse_chord("Ctrl+Shift+Z"),
            Some(Chord::Key(
                Mods {
                    ctrl: true,
                    shift: true,
                    alt: false
                },
                Key::Letter('z')
            ))
        );
        assert_eq!(parse_chord("j"), Some(Chord::plain(Key::Letter('j'))));
        assert_eq!(parse_chord("Escape"), Some(Chord::plain(Key::Escape)));
        assert_eq!(parse_chord("F5"), Some(Chord::plain(Key::F(5))));
        assert_eq!(parse_chord("#"), Some(Chord::Text('#')));
        assert_eq!(parse_chord("/"), Some(Chord::Text('/')));
        assert_eq!(parse_chord("Hyper+K"), None);
        // With Ctrl or Alt a symbol is a key (`Ctrl+,` opens the settings); Shift alone is not.
        assert_eq!(
            parse_chord("Ctrl+,"),
            Some(Chord::Key(
                Mods {
                    ctrl: true,
                    ..Mods::NONE
                },
                Key::Symbol(',')
            ))
        );
        assert_eq!(parse_chord("Shift+#"), None);
        assert_eq!(
            parse_sequence("g i"),
            Some(vec![
                Chord::plain(Key::Letter('g')),
                Chord::plain(Key::Letter('i'))
            ])
        );
    }

    #[test]
    fn typed_characters_become_chords() {
        assert_eq!(Chord::typed('u'), parse_chord("u"));
        assert_eq!(Chord::typed('U'), parse_chord("Shift+U"));
        assert_eq!(Chord::typed('#'), parse_chord("#"));
        assert_eq!(Chord::typed(':'), Some(Chord::Text(':')));
        assert_eq!(Chord::typed('\u{1b}'), None);
    }
}
