//! The UI strings table: every text the front-ends share is a [`Key`] with a template per
//! language, `{name}` placeholders filled by [`tr`].
//!
//! The real languages are English and French. [`Lang::Pseudo`] is a pseudo-locale for
//! developers (`DAIL_LANG=pseudo`): every template accented, a third longer and bracketed, the
//! placeholders kept, so hard-coded strings (plain ASCII) and clipped ones (no closing bracket)
//! stand out. Adding a language: a variant of [`Lang`] and a column in `table.rs`.
//!
//! Command titles have their own table (`commands.rs`). The language of the running app is
//! [`current`]: the front-ends set it from the `language` setting ([`Lang::resolve`]), and code
//! with no [`Lang`] at hand reads it through [`s`], [`f`] and [`title`].

mod commands;
mod engine;
mod table;

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

use app_core::{Category, CommandName};

pub use engine::{error_in, help_in, label_in, message_in};
pub use table::{Key, english, french};

/// A UI language.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Lang {
    /// English.
    #[default]
    English,
    /// French.
    French,
    /// The pseudo-locale: English, accented, longer, bracketed.
    Pseudo,
}

impl Lang {
    /// The language of the `language` setting (`0`: `system`, the language of the system).
    pub const fn resolve(setting: u64, system: Self) -> Self {
        match setting {
            1 => Self::English,
            2 => Self::French,
            _ => system,
        }
    }

    /// The language of a tag such as `$DAIL_LANG` (`fr`, `fr-CA`, `fr_FR.UTF-8`: French;
    /// `pseudo` or `en-XA`: the pseudo-locale; anything else, or nothing: English).
    pub fn from_tag(tag: Option<&str>) -> Self {
        let tag = tag.map(|t| t.trim().to_ascii_lowercase());
        match tag.as_deref() {
            Some("pseudo" | "en-xa") => Self::Pseudo,
            Some(t) if t.split(['-', '_', '.']).next() == Some("fr") => Self::French,
            _ => Self::English,
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

/// The language of the system: `$DAIL_LANG`, else the POSIX locale variables.
pub fn system_lang() -> Lang {
    let tag = ["DAIL_LANG", "LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|name| std::env::var(name).ok().filter(|v| !v.is_empty()));
    Lang::from_tag(tag.as_deref())
}

/// Makes `lang` the language of the running app.
pub fn set_current(lang: Lang) {
    CURRENT.store(lang as u8, Ordering::Relaxed);
}

/// The language of the running app.
pub fn current() -> Lang {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Lang::French,
        2 => Lang::Pseudo,
        _ => Lang::English,
    }
}

/// `key` in the current language.
pub fn s(key: Key) -> &'static str {
    text(current(), key)
}

/// `key` in the current language with its placeholders filled.
pub fn f(key: Key, args: &[(&str, &str)]) -> String {
    tr(current(), key, args)
}

/// The engine error `err` in the current language.
pub fn error(err: &app_core::CoreError) -> String {
    error_in(current(), err)
}

/// The error text `text` in the current language. See [`message_in`].
pub fn message(text: &str) -> String {
    message_in(current(), text)
}

/// The undo label `label` in the current language.
pub fn label(label: &str) -> String {
    label_in(current(), label)
}

/// The one-sentence help of `name` in the current language.
pub fn help(name: CommandName) -> String {
    help_in(current(), name)
}

/// The title of `name` in `lang`.
pub fn title_in(lang: Lang, name: CommandName) -> &'static str {
    static PSEUDO: OnceLock<Vec<String>> = OnceLock::new();
    match lang {
        Lang::English => name.spec().title,
        Lang::French => commands::french(name),
        Lang::Pseudo => CommandName::ALL
            .iter()
            .position(|&n| n == name)
            .and_then(|at| {
                PSEUDO
                    .get_or_init(|| {
                        CommandName::ALL
                            .iter()
                            .map(|n| pseudo(n.spec().title))
                            .collect()
                    })
                    .get(at)
            })
            .map_or_else(|| name.spec().title, String::as_str),
    }
}

/// The title of `name` in the current language.
pub fn title(name: CommandName) -> &'static str {
    title_in(current(), name)
}

/// The label of a palette category in the current language.
pub const fn category_key(category: Category) -> Key {
    match category {
        Category::Mail => Key::Mail,
        Category::Navigation => Key::Navigation,
        Category::Compose => Key::Compose,
        Category::Account => Key::Account,
        Category::App => Key::App,
    }
}

/// The template of `key` in `lang`.
pub fn text(lang: Lang, key: Key) -> &'static str {
    static PSEUDO: OnceLock<Vec<String>> = OnceLock::new();
    match lang {
        Lang::English => english(key),
        Lang::French => french(key),
        Lang::Pseudo => PSEUDO
            .get_or_init(|| Key::ALL.iter().map(|&k| pseudo(english(k))).collect())
            .get(key as usize)
            .map_or_else(|| english(key), String::as_str),
    }
}

/// `key` in `lang` with its placeholders filled from `args` (`(name, value)`; unknown
/// placeholders stay as they are).
pub fn tr(lang: Lang, key: Key, args: &[(&str, &str)]) -> String {
    let mut text = self::text(lang, key).to_owned();
    for (name, value) in args {
        text = text.replace(&format!("{{{name}}}"), value);
    }
    text
}

/// The pseudo-locale of an English template: letters accented outside placeholders, a third
/// longer, in brackets.
fn pseudo(english: &str) -> String {
    let mut out = String::from("[");
    let mut in_placeholder = false;
    let mut letters = 0_usize;
    for c in english.chars() {
        match c {
            '{' => in_placeholder = true,
            '}' => in_placeholder = false,
            _ => {}
        }
        if in_placeholder || c == '}' {
            out.push(c);
            continue;
        }
        if c.is_ascii_alphabetic() {
            letters += 1;
        }
        out.push(accented(c));
    }
    out.push(' ');
    out.extend(std::iter::repeat_n('·', letters.div_ceil(3)));
    out.push(']');
    out
}

const fn accented(c: char) -> char {
    match c {
        'a' => 'á',
        'e' => 'é',
        'i' => 'í',
        'o' => 'ö',
        'u' => 'ü',
        'c' => 'ç',
        'n' => 'ñ',
        's' => 'š',
        'y' => 'ý',
        'A' => 'Å',
        'E' => 'É',
        'I' => 'Î',
        'O' => 'Ø',
        'U' => 'Ü',
        'C' => 'Ç',
        'N' => 'Ñ',
        'S' => 'Š',
        c => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placeholders(text: &str) -> Vec<&str> {
        text.split('{')
            .skip(1)
            .filter_map(|rest| rest.split_once('}').map(|(name, _)| name))
            .collect()
    }

    #[test]
    fn every_key_has_the_same_placeholders_in_every_language() {
        for &key in Key::ALL {
            let english = text(Lang::English, key);
            let pseudo = text(Lang::Pseudo, key);
            assert_eq!(placeholders(english), placeholders(pseudo), "{key:?}");
            assert_eq!(placeholders(english), placeholders(french(key)), "{key:?}");
            assert!(pseudo.starts_with('[') && pseudo.ends_with(']'), "{key:?}");
            assert!(pseudo.chars().count() > english.chars().count(), "{key:?}");
            assert!(!english.is_empty());
        }
    }

    #[test]
    fn french_is_a_real_table() {
        assert_eq!(text(Lang::French, Key::Compose), "Rédiger");
        assert_eq!(
            tr(Lang::French, Key::SearchTitle, &[("query", "dail")]),
            "Recherche : dail"
        );
        assert!(Key::ALL.iter().all(|&key| !french(key).is_empty()));
    }

    #[test]
    fn placeholders_are_filled() {
        assert_eq!(
            tr(
                Lang::English,
                Key::PendingFailed,
                &[("pending", "3"), ("failed", "1")]
            ),
            "3 pending · 1 failed"
        );
        assert_eq!(
            tr(
                Lang::Pseudo,
                Key::LoginFailed,
                &[("email", "me@example.org")]
            ),
            "[me@example.org: lögíñ fáíléd ····]"
        );
        assert_eq!(Lang::from_tag(Some(" en-XA ")), Lang::Pseudo);
        assert_eq!(Lang::from_tag(Some("fr_CA.UTF-8")), Lang::French);
        assert_eq!(Lang::from_tag(Some("fr")), Lang::French);
        assert_eq!(Lang::from_tag(Some("fy")), Lang::English);
        assert_eq!(Lang::from_tag(Some("de_DE.UTF-8")), Lang::English);
        assert_eq!(Lang::from_tag(None), Lang::English);
    }
}
