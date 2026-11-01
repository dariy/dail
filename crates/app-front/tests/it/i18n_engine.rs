//! Engine texts for display: English unchanged, French translated (MISC-32).

use app_core::{CommandName, CoreError};
use app_front::i18n::{Lang, error_in, help_in, label_in};

#[test]
fn every_command_has_a_french_help() {
    for &name in CommandName::ALL {
        let french = help_in(Lang::French, name);
        assert!(!french.is_empty(), "{name:?}");
        assert_ne!(french, help_in(Lang::English, name), "{name:?}");
    }
}

#[test]
fn errors_are_translated_and_english_is_the_engines() {
    let err = CoreError::InvalidArgument {
        field: "password".to_owned(),
        reason: "must not be empty".to_owned(),
    };
    assert_eq!(error_in(Lang::English, &err), err.to_string());
    assert_eq!(
        error_in(Lang::French, &err),
        "argument `password` non valide : ne doit pas être vide"
    );
    assert_eq!(
        error_in(Lang::French, &CoreError::NothingToUndo),
        "rien à annuler"
    );
}

#[test]
fn undo_labels_keep_their_values() {
    for (english, french) in [
        ("Archive (3 messages)", "Archiver (3 messages)"),
        (
            "Mark unread (1 message)",
            "Marquer comme non lu (1 message)",
        ),
        ("Set poll_secs", "Régler poll_secs"),
        ("Theme", "Thème"),
        (
            "Keep the newest 50 of Work offline",
            "Garder hors ligne les 50 plus récents de Work",
        ),
        ("Keep all of Work offline", "Tout garder hors ligne : Work"),
        ("Something new", "Something new"),
    ] {
        assert_eq!(label_in(Lang::French, english), french);
        assert_eq!(label_in(Lang::English, english), english);
    }
}
