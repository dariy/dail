//! Account commands: happy paths, every error variant, undo/redo, events and secret redaction.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::{
    AccountId, AccountStatus, AppState, Command, CommandKind, CoreError, Ctx, Effect, Endpoint,
    Engine, EntityRef, Event, Origin, PersistDelta, ProviderPreset, Secret, SecretRef, Security,
};
use serde_json::json;

const SEED: u128 = 0x1000_0000_0000_0000;

fn ctx() -> Ctx {
    Ctx {
        now_ms: 1_700_000_000_000,
        id_seed: SEED,
        origin: Origin::Cli,
    }
}

fn ctx_seed(id_seed: u128) -> Ctx {
    Ctx { id_seed, ..ctx() }
}

fn first_id(seed: u128) -> AccountId {
    AccountId::from_uuid(uuid::Uuid::from_u128(seed))
}

fn add(preset: ProviderPreset, email: &str) -> Command {
    Command::AddAccount {
        preset,
        email: email.to_owned(),
        display_name: None,
        password: Some(Secret::new("hunter2")),
        imap: None,
        smtp: None,
    }
}

fn endpoint(host: &str, port: u16, security: Security) -> Endpoint {
    Endpoint {
        host: host.to_owned(),
        port,
        security,
    }
}

fn engine_with_gmail() -> (Engine, AccountId) {
    let mut engine = Engine::default();
    engine
        .dispatch(add(ProviderPreset::Gmail, "me@gmail.com"), &ctx())
        .unwrap();
    (engine, first_id(SEED))
}

#[test]
fn add_account_applies_preset_and_stores_secret_in_keyring_only() {
    let mut engine = Engine::default();
    let outcome = engine
        .dispatch(add(ProviderPreset::Gmail, " me@gmail.com "), &ctx())
        .unwrap();
    let id = first_id(SEED);
    let account = engine.state().accounts.get(&id).unwrap().clone();
    assert_eq!(account.email, "me@gmail.com");
    assert_eq!(account.display_name, "me@gmail.com");
    assert_eq!(account.imap, endpoint("imap.gmail.com", 993, Security::Tls));
    assert_eq!(account.smtp, endpoint("smtp.gmail.com", 465, Security::Tls));
    assert_eq!(account.secret, SecretRef::for_account(id));
    assert_eq!(account.status, AccountStatus::Unverified);

    assert_eq!(outcome.receipt.kind, Some(CommandKind::Mutation));
    assert_eq!(outcome.receipt.affected, vec![EntityRef::Account(id)]);
    assert_eq!(
        outcome.receipt.undo_label.as_deref(),
        Some("Add account me@gmail.com")
    );
    assert_eq!(
        outcome.effects,
        vec![
            Effect::PersistState {
                delta: PersistDelta::Account(account.clone())
            },
            Effect::StoreSecret {
                account: id,
                secret: account.secret.clone(),
                value: Secret::new("hunter2"),
            },
        ]
    );

    // The password never shows up in state, receipts or serialized effects.
    let dumps = [
        serde_json::to_string(engine.state()).unwrap(),
        serde_json::to_string(&outcome.receipt).unwrap(),
        serde_json::to_string(&outcome.effects).unwrap(),
        format!("{outcome:?}"),
    ];
    for dump in dumps {
        assert!(!dump.contains("hunter2"), "secret leaked: {dump}");
    }
}

#[test]
fn add_account_without_password_emits_no_keyring_effect() {
    let mut engine = Engine::default();
    let cmd = Command::AddAccount {
        preset: ProviderPreset::Hostinger,
        email: "me@example.com".into(),
        display_name: Some("  Work  ".into()),
        password: None,
        imap: None,
        smtp: None,
    };
    let outcome = engine.dispatch(cmd, &ctx()).unwrap();
    let account = &engine.state().accounts[&first_id(SEED)];
    assert_eq!(account.display_name, "Work");
    assert_eq!(account.imap.host, "imap.hostinger.com");
    assert_eq!(outcome.effects.len(), 1);
    assert!(matches!(outcome.effects[0], Effect::PersistState { .. }));
}

#[test]
fn custom_preset_takes_explicit_servers() {
    let mut engine = Engine::default();
    let cmd = Command::AddAccount {
        preset: ProviderPreset::Custom,
        email: "me@example.org".into(),
        display_name: None,
        password: None,
        imap: Some(endpoint("mail.example.org", 143, Security::StartTls)),
        smtp: Some(endpoint("mail.example.org", 587, Security::StartTls)),
    };
    engine.dispatch(cmd, &ctx()).unwrap();
    let account = &engine.state().accounts[&first_id(SEED)];
    assert_eq!(account.imap.security, Security::StartTls);
    assert_eq!(account.smtp.port, 587);
}

#[test]
fn add_account_rejects_bad_input_without_changing_state() {
    let cases: Vec<(Command, &str)> = vec![
        (add(ProviderPreset::Gmail, "not-an-address"), "email"),
        (add(ProviderPreset::Gmail, "a@b@c.com"), "email"),
        (add(ProviderPreset::Gmail, "me @gmail.com"), "email"),
        (add(ProviderPreset::Custom, "me@example.org"), "imap"),
        (
            Command::AddAccount {
                preset: ProviderPreset::Custom,
                email: "me@example.org".into(),
                display_name: None,
                password: None,
                imap: Some(endpoint("mail.example.org", 993, Security::Tls)),
                smtp: None,
            },
            "smtp",
        ),
        (
            Command::AddAccount {
                preset: ProviderPreset::Gmail,
                email: "me@gmail.com".into(),
                display_name: None,
                password: None,
                imap: Some(endpoint("bad host", 993, Security::Tls)),
                smtp: None,
            },
            "imap",
        ),
        (
            Command::AddAccount {
                preset: ProviderPreset::Gmail,
                email: "me@gmail.com".into(),
                display_name: None,
                password: None,
                imap: None,
                smtp: Some(endpoint("smtp.gmail.com", 0, Security::Tls)),
            },
            "smtp",
        ),
        (
            Command::AddAccount {
                preset: ProviderPreset::Gmail,
                email: "me@gmail.com".into(),
                display_name: Some("   ".into()),
                password: None,
                imap: None,
                smtp: None,
            },
            "display_name",
        ),
        (
            Command::AddAccount {
                preset: ProviderPreset::Gmail,
                email: "me@gmail.com".into(),
                display_name: None,
                password: Some(Secret::new("")),
                imap: None,
                smtp: None,
            },
            "password",
        ),
    ];
    for (cmd, field) in cases {
        let mut engine = Engine::default();
        let err = engine.dispatch(cmd.clone(), &ctx()).unwrap_err();
        assert!(
            matches!(err, CoreError::InvalidArgument { field: ref f, .. } if f == field),
            "{cmd:?} → {err:?}"
        );
        assert_eq!(engine.state(), &AppState::default());
        assert_eq!(engine.history().undo_len(), 0);
    }
}

#[test]
fn duplicate_email_is_already_exists() {
    let (mut engine, _) = engine_with_gmail();
    let before = engine.state().clone();
    let err = engine
        .dispatch(
            add(ProviderPreset::Gmail, "ME@gmail.com"),
            &ctx_seed(2 << 64),
        )
        .unwrap_err();
    assert_eq!(
        serde_json::to_value(&err).unwrap(),
        json!({"error": "already_exists", "entity": "account", "key": "ME@gmail.com"})
    );
    assert_eq!(engine.state(), &before);
}

#[test]
fn colliding_seed_is_already_exists() {
    let (mut engine, id) = engine_with_gmail();
    let err = engine
        .dispatch(add(ProviderPreset::Hostinger, "other@example.com"), &ctx())
        .unwrap_err();
    assert_eq!(
        err,
        CoreError::AlreadyExists {
            entity: "account".into(),
            key: id.to_string()
        }
    );
}

#[test]
fn add_undo_redo_round_trip_keeps_the_id() {
    let mut engine = Engine::default();
    let initial = engine.state().clone();
    engine
        .dispatch(add(ProviderPreset::Gmail, "me@gmail.com"), &ctx())
        .unwrap();
    let added = engine.state().clone();
    let id = first_id(SEED);

    let undo = engine
        .dispatch(Command::Undo {}, &ctx_seed(5 << 64))
        .unwrap();
    assert_eq!(engine.state(), &initial);
    assert_eq!(
        undo.receipt.data,
        json!({"undone": "Add account me@gmail.com"})
    );
    assert!(undo.effects.contains(&Effect::DeleteSecret {
        account: id,
        secret: SecretRef::for_account(id),
    }));
    assert!(undo.effects.contains(&Effect::PersistState {
        delta: PersistDelta::AccountRemoved(id),
    }));

    // Redo runs with a different ctx seed but must recreate the same ID (and the secret).
    let redo = engine
        .dispatch(Command::Redo {}, &ctx_seed(9 << 64))
        .unwrap();
    assert_eq!(engine.state(), &added);
    assert!(redo.effects.iter().any(|effect| matches!(
        effect,
        Effect::StoreSecret { account, .. } if *account == id
    )));
}

#[test]
fn test_account_emits_verify_effect_without_mutating() {
    let (mut engine, id) = engine_with_gmail();
    let before = engine.state().clone();
    let outcome = engine
        .dispatch(Command::TestAccount { id }, &ctx())
        .unwrap();
    assert_eq!(outcome.receipt.kind, Some(CommandKind::Query));
    assert_eq!(outcome.receipt.undo_label, None);
    assert_eq!(
        outcome.effects,
        vec![Effect::VerifyAccount {
            account: id,
            login: "me@gmail.com".into(),
            imap: endpoint("imap.gmail.com", 993, Security::Tls),
            smtp: endpoint("smtp.gmail.com", 465, Security::Tls),
            secret: SecretRef::for_account(id),
        }]
    );
    assert_eq!(engine.state(), &before);
    assert_eq!(
        engine.history().undo_len(),
        1,
        "only add_account is recorded"
    );
}

#[test]
fn unknown_account_is_not_found() {
    let mut engine = Engine::default();
    let id = first_id(77);
    let commands = [
        Command::TestAccount { id },
        Command::UpdateAccount {
            id,
            display_name: Some("x".into()),
            imap: None,
            smtp: None,
        },
        Command::RemoveAccount { id, confirm: true },
    ];
    for cmd in commands {
        let err = engine.dispatch(cmd, &ctx()).unwrap_err();
        assert_eq!(
            serde_json::to_value(&err).unwrap(),
            json!({"error": "not_found", "entity": "account", "id": id.to_string()})
        );
    }
}

#[test]
fn verification_events_update_status() {
    let (mut engine, id) = engine_with_gmail();
    let later = Ctx {
        now_ms: 1_700_000_005_000,
        ..ctx()
    };
    let outcome = engine
        .ingest(
            Event::AccountVerifyFailed {
                account: id,
                error: "IMAP LOGIN failed".into(),
            },
            &later,
        )
        .unwrap();
    assert_eq!(outcome.receipt.affected, vec![EntityRef::Account(id)]);
    assert_eq!(
        engine.state().accounts[&id].status,
        AccountStatus::Failed {
            error: "IMAP LOGIN failed".into(),
            at_ms: 1_700_000_005_000
        }
    );
    engine
        .ingest(Event::AccountVerified { account: id }, &later)
        .unwrap();
    assert_eq!(
        engine.state().accounts[&id].status,
        AccountStatus::Verified {
            at_ms: 1_700_000_005_000
        }
    );
}

#[test]
fn set_password_stores_it_and_tests_the_login() {
    let (mut engine, id) = engine_with_gmail();
    engine
        .ingest(
            Event::AccountVerifyFailed {
                account: id,
                error: "IMAP LOGIN failed".into(),
            },
            &ctx(),
        )
        .unwrap();
    let undo_before = engine.history().undo_len();
    let outcome = engine
        .dispatch(
            Command::SetPassword {
                id,
                password: Secret::new("new app password"),
            },
            &ctx(),
        )
        .unwrap();
    let account = engine.state().accounts[&id].clone();
    assert_eq!(account.status, AccountStatus::Unverified);
    let [
        Effect::PersistState { .. },
        Effect::StoreSecret {
            account: stored,
            secret,
            value,
        },
        Effect::VerifyAccount {
            account: tested, ..
        },
    ] = &outcome.effects[..]
    else {
        panic!("{:?}", outcome.effects)
    };
    assert_eq!((*stored, *tested), (id, id));
    assert_eq!(*secret, account.secret);
    assert_eq!(value.expose(), "new app password");
    // Never in the receipt, the state or the effect's JSON.
    let json = format!(
        "{} {} {}",
        serde_json::to_string(&outcome.receipt).unwrap(),
        serde_json::to_string(engine.state()).unwrap(),
        serde_json::to_string(&outcome.effects).unwrap()
    );
    assert!(!json.contains("new app password"), "{json}");
    assert_eq!(engine.history().undo_len(), undo_before, "not undoable");
}

#[test]
fn set_password_refuses_empty_passwords_unknown_accounts_and_agents() {
    let (mut engine, id) = engine_with_gmail();
    let err = engine
        .dispatch(
            Command::SetPassword {
                id,
                password: Secret::new(""),
            },
            &ctx(),
        )
        .unwrap_err();
    assert!(matches!(err, CoreError::InvalidArgument { ref field, .. } if field == "password"));
    let err = engine
        .dispatch(
            Command::SetPassword {
                id: first_id(SEED + (1 << 64)),
                password: Secret::new("x"),
            },
            &ctx(),
        )
        .unwrap_err();
    assert!(matches!(err, CoreError::NotFound { .. }), "{err:?}");
    let agent = Ctx {
        origin: Origin::Mcp,
        ..ctx()
    };
    let err = engine
        .dispatch(
            Command::SetPassword {
                id,
                password: Secret::new("x"),
            },
            &agent,
        )
        .unwrap_err();
    assert!(matches!(err, CoreError::OriginNotAllowed { .. }), "{err:?}");
}

#[test]
fn keyring_failures_surface_in_state() {
    let (mut engine, id) = engine_with_gmail();
    engine
        .ingest(
            Event::SecretFailed {
                account: id,
                error: "no secret service".into(),
            },
            &ctx(),
        )
        .unwrap();
    assert_eq!(
        engine.state().status.keyring_error.as_deref(),
        Some(format!("account {id}: no secret service").as_str())
    );
    assert!(matches!(
        &engine.state().accounts[&id].status,
        AccountStatus::Failed { error, .. } if error == "keyring: no secret service"
    ));
    engine
        .ingest(Event::SecretStored { account: id }, &ctx())
        .unwrap();
    assert_eq!(engine.state().status.keyring_error, None);
}

#[test]
fn events_for_removed_accounts_are_harmless() {
    let mut engine = Engine::default();
    let ghost = first_id(99);
    let outcome = engine
        .ingest(Event::AccountVerified { account: ghost }, &ctx())
        .unwrap();
    assert!(outcome.receipt.affected.is_empty());
    engine
        .ingest(Event::SecretDeleted { account: ghost }, &ctx())
        .unwrap();
    assert_eq!(engine.state(), &AppState::default());
    // A failed delete for a removed account is still visible.
    engine
        .ingest(
            Event::SecretFailed {
                account: ghost,
                error: "locked".into(),
            },
            &ctx(),
        )
        .unwrap();
    assert!(engine.state().status.keyring_error.is_some());
}

#[test]
fn update_account_changes_fields_and_round_trips() {
    let (mut engine, id) = engine_with_gmail();
    engine
        .ingest(Event::AccountVerified { account: id }, &ctx())
        .unwrap();
    let mut before = engine.state().clone();

    // Renaming keeps the verification status.
    engine
        .dispatch(
            Command::UpdateAccount {
                id,
                display_name: Some("Personal".into()),
                imap: None,
                smtp: None,
            },
            &ctx(),
        )
        .unwrap();
    assert_eq!(engine.state().accounts[&id].display_name, "Personal");
    assert!(matches!(
        engine.state().accounts[&id].status,
        AccountStatus::Verified { .. }
    ));
    engine.dispatch(Command::Undo {}, &ctx()).unwrap();
    assert_eq!(engine.state(), &before);

    // Changing a server resets the status; undo restores the server (status stays unverified).
    let outcome = engine
        .dispatch(
            Command::UpdateAccount {
                id,
                display_name: None,
                imap: Some(endpoint("imap.example.net", 993, Security::Tls)),
                smtp: None,
            },
            &ctx(),
        )
        .unwrap();
    assert_eq!(
        outcome.receipt.undo_label.as_deref(),
        Some("Edit account me@gmail.com")
    );
    let changed = engine.state().clone();
    assert_eq!(changed.accounts[&id].status, AccountStatus::Unverified);
    engine.dispatch(Command::Undo {}, &ctx()).unwrap();
    before.accounts[&id].status = AccountStatus::Unverified;
    assert_eq!(engine.state(), &before);
    engine.dispatch(Command::Redo {}, &ctx()).unwrap();
    assert_eq!(engine.state(), &changed);
}

#[test]
fn update_account_rejects_empty_and_invalid_patches() {
    let (mut engine, id) = engine_with_gmail();
    let before = engine.state().clone();
    let bad = [
        (
            Command::UpdateAccount {
                id,
                display_name: None,
                imap: None,
                smtp: None,
            },
            "display_name",
        ),
        (
            Command::UpdateAccount {
                id,
                display_name: Some("\u{7}".into()),
                imap: None,
                smtp: None,
            },
            "display_name",
        ),
        (
            Command::UpdateAccount {
                id,
                display_name: None,
                imap: None,
                smtp: Some(endpoint("", 465, Security::Tls)),
            },
            "smtp",
        ),
    ];
    for (cmd, field) in bad {
        let err = engine.dispatch(cmd, &ctx()).unwrap_err();
        assert!(
            matches!(err, CoreError::InvalidArgument { field: ref f, .. } if f == field),
            "{err:?}"
        );
        assert_eq!(engine.state(), &before);
    }
}

#[test]
fn remove_account_requires_confirmation_and_is_not_undoable() {
    let (mut engine, id) = engine_with_gmail();
    let before = engine.state().clone();
    let err = engine
        .dispatch(Command::RemoveAccount { id, confirm: false }, &ctx())
        .unwrap_err();
    assert_eq!(
        serde_json::to_value(&err).unwrap(),
        json!({"error": "confirmation_required", "command": "remove_account"})
    );
    assert_eq!(engine.state(), &before);

    let outcome = engine
        .dispatch(Command::RemoveAccount { id, confirm: true }, &ctx())
        .unwrap();
    assert_eq!(outcome.receipt.kind, Some(CommandKind::Destructive));
    assert_eq!(outcome.receipt.undo_label, None);
    assert_eq!(outcome.receipt.data["undoable"], false);
    assert!(engine.state().accounts.is_empty());
    assert_eq!(
        outcome.effects,
        vec![
            Effect::PersistState {
                delta: PersistDelta::AccountRemoved(id)
            },
            Effect::DeleteSecret {
                account: id,
                secret: SecretRef::for_account(id)
            },
        ]
    );
    // Only add_account is in the history; undoing it now finds nothing to remove.
    assert_eq!(engine.history().undo_len(), 1);
}

#[test]
fn remove_account_confirm_defaults_to_false() {
    let cmd =
        Command::from_value(json!({"cmd": "remove_account", "args": {"id": first_id(1)}})).unwrap();
    assert_eq!(
        cmd,
        Command::RemoveAccount {
            id: first_id(1),
            confirm: false
        }
    );
}

#[test]
fn secrets_are_redacted_in_debug_and_json_but_parse_from_json() {
    let cmd = Command::from_value(json!({
        "cmd": "add_account",
        "args": {"preset": "gmail", "email": "me@gmail.com", "password": "hunter2"}
    }))
    .unwrap();
    let password = match &cmd {
        Command::AddAccount { password, .. } => password.clone(),
        _ => None,
    };
    assert_eq!(password.as_ref().map(Secret::expose), Some("hunter2"));
    assert!(!format!("{cmd:?}").contains("hunter2"));
    let json = serde_json::to_value(&cmd).unwrap();
    assert_eq!(json["args"]["password"], Secret::REDACTED);
}

#[test]
fn preset_profiles_are_data() {
    let gmail = ProviderPreset::Gmail.profile();
    assert!(gmail.smtp_saves_sent);
    assert_eq!(gmail.archive_mailbox, Some("[Gmail]/All Mail"));
    let hostinger = ProviderPreset::Hostinger.profile();
    assert!(!hostinger.smtp_saves_sent);
    assert!(hostinger.create_archive);
    let custom = ProviderPreset::Custom.profile();
    assert!(custom.imap.is_none() && custom.smtp.is_none());
}
