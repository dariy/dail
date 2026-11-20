//! Engine texts for display: English unchanged, French, Spanish, Romanian, Portuguese and Latin translated (MISC-32).

use app_core::{CommandName, CoreError};
use app_front::i18n::{self, Lang, error_in, help_in, label_in};

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
        "ERR002 : argument `password` non valide : ne doit pas être vide"
    );
    assert_eq!(
        error_in(Lang::French, &CoreError::NothingToUndo),
        "ERR010 : rien à annuler"
    );
}

#[test]
fn reasons_with_values_keep_them() {
    for (reason, french) in [
        ("Work exists already", "Work existe déjà"),
        (
            "account a has no folder \"b\"",
            "le compte a n’a pas de dossier \"b\"",
        ),
        (
            "at column 3: invalid date `x`; write YYYY-MM-DD",
            "à la colonne 3 : date `x` non valide ; écrivez AAAA-MM-JJ",
        ),
        ("a server said no", "a server said no"),
    ] {
        let err = CoreError::InvalidArgument {
            field: "f".to_owned(),
            reason: reason.to_owned(),
        };
        assert_eq!(
            error_in(Lang::French, &err),
            format!("ERR002 : argument `f` non valide : {french}")
        );
    }
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

#[test]
fn error_texts_keep_their_code_and_values() {
    let fr = |text| i18n::message_in(Lang::French, text);
    assert_eq!(
        fr("ERR002: invalid argument `keys`: at most 8 keys"),
        "ERR002 : argument `keys` non valide : 8 touches au maximum"
    );
    assert_eq!(fr("ERR207: session closed"), "ERR207 : session fermée");
    assert_eq!(
        fr("ERR004: thread t1 not found"),
        "ERR004 : discussion t1 introuvable"
    );
    assert_eq!(
        fr("ERR201: smtp failed: session closed"),
        "ERR201 : échec de smtp : session fermée"
    );
    assert_eq!(fr("535 denied by server"), "535 denied by server");
    assert_eq!(
        i18n::message_in(Lang::English, "ERR207: session closed"),
        "ERR207: session closed"
    );
}

#[test]
fn the_last_validation_reasons_are_translated() {
    let fr = |text| i18n::message_in(Lang::French, text);
    for text in [
        "expected an address like sales@example.org",
        "at most 4096 bytes",
        "x@y.z is not an address of the account",
        "the body is not downloaded yet (open_thread downloads it)",
        "expected the absolute path of a folder",
        "at most 32 parts",
        "no command is named nope",
        "not a key: \"?\"",
        "give a mode, a light theme or a dark theme",
        "text_scale expects a non-negative integer",
    ] {
        assert_ne!(fr(text), text);
    }
}

#[test]
fn spanish_and_romanian_translate_helps_errors_and_labels() {
    for &name in CommandName::ALL {
        for lang in [
            Lang::Spanish,
            Lang::Romanian,
            Lang::PortugueseBrazil,
            Lang::PortuguesePortugal,
            Lang::Latin,
        ] {
            let help = help_in(lang, name);
            assert!(!help.is_empty(), "{lang:?} {name:?}");
            assert_ne!(help, help_in(Lang::English, name), "{lang:?} {name:?}");
            assert_ne!(
                i18n::title_in(lang, name),
                name.spec().title,
                "{lang:?} {name:?}: a title of its own"
            );
        }
    }
    let err = CoreError::InvalidArgument {
        field: "password".to_owned(),
        reason: "must not be empty".to_owned(),
    };
    assert_eq!(
        error_in(Lang::Spanish, &err),
        "ERR002: argumento `password` no válido: no puede estar vacío"
    );
    assert_eq!(
        error_in(Lang::Romanian, &err),
        "ERR002: argument `password` nevalid: nu trebuie să fie gol"
    );
    let at_column = CoreError::InvalidArgument {
        field: "f".to_owned(),
        reason: "at column 3: invalid date `x`; write YYYY-MM-DD".to_owned(),
    };
    assert_eq!(
        error_in(Lang::Spanish, &at_column),
        "ERR002: argumento `f` no válido: en la columna 3: fecha `x` no válida; escribe AAAA-MM-DD"
    );
    assert_eq!(
        label_in(Lang::Romanian, "Keep the newest 50 of Work offline"),
        "Păstrarea offline a celor mai noi 50 din Work"
    );
    assert_eq!(
        label_in(Lang::Spanish, "Archive (3 messages)"),
        "Archivar (3 messages)"
    );
    // CA-2's texts are translated too.
    assert_eq!(
        label_in(Lang::French, "Trust the certificate"),
        "Faire confiance au certificat"
    );
    let not_found = CoreError::NotFound {
        entity: "certificate offer".to_owned(),
        id: "ab".to_owned(),
    };
    assert_eq!(
        error_in(Lang::Spanish, &not_found),
        "ERR004: certificado ofrecido ab: no se encontró"
    );
}

#[test]
fn portuguese_and_latin_translate_errors_and_labels() {
    let err = CoreError::InvalidArgument {
        field: "password".to_owned(),
        reason: "must not be empty".to_owned(),
    };
    assert_eq!(
        error_in(Lang::PortugueseBrazil, &err),
        "ERR002: argumento `password` inválido: não pode estar vazio"
    );
    assert_eq!(
        error_in(Lang::Latin, &err),
        "ERR002: argumentum `password` invalidum: vacuum esse non debet"
    );
    assert_eq!(
        error_in(Lang::PortuguesePortugal, &CoreError::NothingToUndo),
        "ERR010: nada para anular"
    );
    assert_eq!(
        error_in(Lang::PortugueseBrazil, &CoreError::NothingToUndo),
        "ERR010: nada para desfazer"
    );
    assert_eq!(
        label_in(Lang::PortuguesePortugal, "Rename Work"),
        "Mudar o nome de Work"
    );
    assert_eq!(
        label_in(Lang::Latin, "Archive (3 messages)"),
        "In archivum condere (3 messages)"
    );
}
