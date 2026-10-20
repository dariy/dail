//! The UI strings table: every text the front-ends share is a [`Key`] with a template per
//! language, `{name}` placeholders filled by [`tr`].
//!
//! English is the only real language so far. [`Lang::Pseudo`] is a pseudo-locale for
//! developers (`DAIL_LANG=pseudo`): every template accented, a third longer and bracketed, the
//! placeholders kept, so hard-coded strings (plain ASCII) and clipped ones (no closing bracket)
//! stand out. Adding a language: a variant of [`Lang`] and a table like [`english`].
//!
//! Only the texts of `app_front::status` are in the table yet; the GUI's and TUI's own labels
//! move over item by item.

/// A UI language.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Lang {
    /// English.
    #[default]
    English,
    /// The pseudo-locale: English, accented, longer, bracketed.
    Pseudo,
}

impl Lang {
    /// The language of a tag such as `$DAIL_LANG` (`pseudo` or `en-XA`: the pseudo-locale;
    /// anything else, or nothing: English).
    pub fn from_tag(tag: Option<&str>) -> Self {
        match tag.map(|t| t.trim().to_ascii_lowercase()).as_deref() {
            Some("pseudo" | "en-xa") => Self::Pseudo,
            _ => Self::English,
        }
    }
}

/// A string of the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// The store could not be written.
    StoreFailed,
    /// The keyring failed.
    KeyringFailed,
    /// What to do about the keyring.
    KeyringHint,
    /// `{email}`'s login failed.
    LoginFailed,
    /// `{email}`'s sync failed.
    SyncFailed,
    /// The fix: enter the password again.
    FixPassword,
    /// The fix: sync again.
    FixRetry,
    /// The fix: test the login again.
    FixTest,
    /// Gmail refused the password.
    HintPasswordGmail,
    /// A server refused the password.
    HintPassword,
    /// The mailbox is full.
    HintQuota,
    /// No TLS ciphers in common.
    HintTlsCiphers,
    /// An untrusted certificate.
    HintCertificate,
    /// The server could not be reached.
    HintNetwork,
    /// A sync is running.
    Syncing,
    /// Synced `{age}` ago.
    Synced,
    /// There are no accounts.
    NoAccounts,
    /// Never synced.
    NotSynced,
    /// `{pending}` changes wait for the server.
    Pending,
    /// `{pending}` wait, `{failed}` of them failed.
    PendingFailed,
}

impl Key {
    /// Every key.
    pub const ALL: &'static [Self] = &[
        Self::StoreFailed,
        Self::KeyringFailed,
        Self::KeyringHint,
        Self::LoginFailed,
        Self::SyncFailed,
        Self::FixPassword,
        Self::FixRetry,
        Self::FixTest,
        Self::HintPasswordGmail,
        Self::HintPassword,
        Self::HintQuota,
        Self::HintTlsCiphers,
        Self::HintCertificate,
        Self::HintNetwork,
        Self::Syncing,
        Self::Synced,
        Self::NoAccounts,
        Self::NotSynced,
        Self::Pending,
        Self::PendingFailed,
    ];
}

/// The English table.
pub const fn english(key: Key) -> &'static str {
    match key {
        Key::StoreFailed => "local store failed",
        Key::KeyringFailed => "keyring failed",
        Key::KeyringHint => {
            "The system keyring (Secret Service on Linux, Credential Manager on Windows) is \
             locked or missing: unlock it, or start a keyring service."
        }
        Key::LoginFailed => "{email}: login failed",
        Key::SyncFailed => "{email}: sync failed",
        Key::FixPassword => "re-enter password",
        Key::FixRetry => "retry",
        Key::FixTest => "test again",
        Key::HintPasswordGmail => {
            "Gmail refused the password. It needs an App Password (Google account → Security → \
             2-Step Verification → App passwords), not your Google password."
        }
        Key::HintPassword => {
            "The server refused the password (or none is stored). Enter it again; if you \
             changed it on the web, use the new one."
        }
        Key::HintQuota => {
            "The mailbox is full. Delete or archive mail and empty the Trash, then retry."
        }
        Key::HintTlsCiphers => {
            "The server only offers TLS ciphers dail does not have yet (AES-GCM or P-256 key \
             exchange). Nothing to fix on your side; see ROADMAP.md (TLS tiers 2 and 3)."
        }
        Key::HintCertificate => {
            "The server's certificate is not trusted: expired, self-signed or for another \
             name. Check the server name; dail does not connect without a valid certificate."
        }
        Key::HintNetwork => {
            "The server could not be reached. Check the connection and the server name; \
             syncing tries again by itself."
        }
        Key::Syncing => "⟳ syncing…",
        Key::Synced => "synced {age}",
        Key::NoAccounts => "no accounts",
        Key::NotSynced => "not synced",
        Key::Pending => "{pending} pending",
        Key::PendingFailed => "{pending} pending · {failed} failed",
    }
}

/// The template of `key` in `lang`.
pub fn template(lang: Lang, key: Key) -> String {
    match lang {
        Lang::English => english(key).to_owned(),
        Lang::Pseudo => pseudo(english(key)),
    }
}

/// `key` in `lang` with its placeholders filled from `args` (`(name, value)`; unknown
/// placeholders stay as they are).
pub fn tr(lang: Lang, key: Key, args: &[(&str, &str)]) -> String {
    let mut text = template(lang, key);
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
            let english = template(Lang::English, key);
            let pseudo = template(Lang::Pseudo, key);
            assert_eq!(placeholders(&english), placeholders(&pseudo), "{key:?}");
            assert!(pseudo.starts_with('[') && pseudo.ends_with(']'), "{key:?}");
            assert!(pseudo.chars().count() > english.chars().count(), "{key:?}");
            assert!(!english.is_empty());
        }
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
        assert_eq!(Lang::from_tag(Some("de_DE.UTF-8")), Lang::English);
        assert_eq!(Lang::from_tag(None), Lang::English);
    }
}
