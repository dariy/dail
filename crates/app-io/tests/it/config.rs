//! `config.toml`, `ConfigStore` and profile directories.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use app_core::{
    Account, AccountId, AccountStatus, AppState, Endpoint, PersistDelta, ProviderPreset, SecretRef,
    Security, Settings,
};
use app_io::{Config, ConfigStore, IoError, ProfileDirs, Store};

/// A fresh, empty directory under the system temp dir, unique per test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dail-io-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn account(n: u128, email: &str) -> Account {
    let id = AccountId::from_uuid(uuid::Uuid::from_u128(n));
    Account {
        id,
        display_name: format!("Account {n}"),
        email: email.to_owned(),
        preset: ProviderPreset::Custom,
        imap: Endpoint {
            host: "mail.example.org".into(),
            port: 143,
            security: Security::StartTls,
        },
        smtp: Endpoint {
            host: "mail.example.org".into(),
            port: 465,
            security: Security::Tls,
        },
        secret: SecretRef::for_account(id),
        status: AccountStatus::Verified { at_ms: 5 },
        identities: Vec::new(),
    }
}

fn state_with_accounts() -> AppState {
    let mut state = AppState::default();
    state.settings.poll_secs = 300;
    state
        .settings
        .images_from
        .insert("news@example.org".to_owned());
    state.settings.key_preset = app_core::KeyPreset::Vim;
    state
        .settings
        .keys
        .insert("archive_threads".to_owned(), vec!["y".to_owned()]);
    for mut account in [account(1, "a@example.org"), account(2, "b@example.org")] {
        if account.email == "a@example.org" {
            account.identities.push(app_core::Identity {
                email: "sales@example.org".into(),
                name: Some("Sales".into()),
                signature: Some("Sales team\nExample".into()),
            });
        }
        state.accounts.insert(account.id, account);
    }
    state
}

#[test]
fn config_round_trips_and_drops_runtime_status() {
    let state = state_with_accounts();
    let config = Config::from_state(&state);
    let text = config.to_toml().unwrap();
    insta::assert_snapshot!("config_toml", text);
    assert!(!text.to_lowercase().contains("password ="));

    let parsed = Config::parse(&text, "config.toml".as_ref()).unwrap();
    assert_eq!(parsed, config);
    let mut restored = AppState::default();
    parsed.apply_to(&mut restored);
    assert_eq!(restored.settings, state.settings);
    let ids: Vec<_> = restored.accounts.keys().collect();
    assert_eq!(ids, state.accounts.keys().collect::<Vec<_>>(), "order kept");
    assert!(
        restored
            .accounts
            .values()
            .all(|a| a.status == AccountStatus::Unverified)
    );
}

#[test]
fn partial_and_empty_configs_take_defaults() {
    let config = Config::parse("", "c.toml".as_ref()).unwrap();
    assert_eq!(config, Config::default());
    let config = Config::parse("[settings]\npoll_secs = 60\n", "c.toml".as_ref()).unwrap();
    assert_eq!(
        config.settings,
        Settings {
            poll_secs: 60,
            ..Settings::default()
        }
    );
}

#[test]
fn invalid_configs_are_rejected_with_the_path() {
    let bad = [
        "settings = [",
        "[settings]\npoll_secs = 1\n",
        "[settings]\npoll_secs = -5\n",
    ];
    for text in bad {
        let err = Config::parse(text, "/x/config.toml".as_ref()).unwrap_err();
        assert!(
            matches!(err, IoError::Config { ref path, .. } if path == "/x/config.toml"),
            "{text}: {err}"
        );
    }

    let mut config = Config::from_state(&state_with_accounts());
    config.accounts[1].email = "A@example.org".into();
    let err = Config::parse(&config.to_toml().unwrap(), "c.toml".as_ref()).unwrap_err();
    assert!(err.to_string().contains("duplicate account email"), "{err}");

    let mut config = Config::from_state(&state_with_accounts());
    config.accounts[1].id = config.accounts[0].id;
    let err = Config::parse(&config.to_toml().unwrap(), "c.toml".as_ref()).unwrap_err();
    assert!(err.to_string().contains("duplicate account id"), "{err}");
}

#[test]
fn load_missing_is_none_and_save_is_atomic() {
    let dir = scratch("save");
    let path = dir.join("config.toml");
    assert_eq!(Config::load(&path).unwrap(), None);
    let config = Config::from_state(&state_with_accounts());
    config.save(&path).unwrap();
    assert!(!dir.join("config.toml.tmp").exists());
    assert_eq!(Config::load(&path).unwrap(), Some(config));

    let err = Config::default()
        .save(&dir.join("missing-dir/config.toml"))
        .unwrap_err();
    assert!(matches!(err, IoError::Fs { .. }), "{err}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn config_store_applies_deltas_and_reloads() {
    let dir = scratch("store");
    let path = dir.join("config.toml");
    let store = ConfigStore::open(&path).unwrap();
    assert_eq!(store.load_snapshot().unwrap(), None);

    let a = account(1, "a@example.org");
    let b = account(2, "b@example.org");
    store.apply(&PersistDelta::Account(a.clone())).unwrap();
    store.apply(&PersistDelta::Account(b.clone())).unwrap();
    let renamed = Account {
        display_name: "Renamed".into(),
        ..a.clone()
    };
    store.apply(&PersistDelta::Account(renamed)).unwrap();
    store.apply(&PersistDelta::AccountRemoved(b.id)).unwrap();
    store
        .apply(&PersistDelta::Settings(Settings {
            history_cap: 50,
            ..Settings::default()
        }))
        .unwrap();

    let reopened = ConfigStore::open(&path).unwrap();
    let state = reopened.load_snapshot().unwrap().unwrap();
    assert_eq!(state.settings.history_cap, 50);
    assert_eq!(state.accounts.len(), 1);
    assert_eq!(state.accounts[&a.id].display_name, "Renamed");
    assert_eq!(state, store.load_snapshot().unwrap().unwrap());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn config_store_open_fails_on_invalid_file() {
    let dir = scratch("invalid");
    let path = dir.join("config.toml");
    std::fs::write(&path, "not = [valid").unwrap();
    assert!(matches!(
        ConfigStore::open(&path),
        Err(IoError::Config { .. })
    ));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn profile_dirs_layout_and_names() {
    let profile = ProfileDirs::at("/p");
    assert_eq!(profile.config_file(), PathBuf::from("/p/config.toml"));
    assert_eq!(profile.state_db(), PathBuf::from("/p/state.redb"));
    assert_eq!(profile.blobs_dir(), PathBuf::from("/p/blobs"));
    assert_eq!(profile.logs_dir(), PathBuf::from("/p/logs"));
    assert_eq!(profile.lock_file(), PathBuf::from("/p/lock"));

    for bad in ["", "../etc", "a/b", "white space", "ü"] {
        assert!(
            matches!(ProfileDirs::named(bad), Err(IoError::InvalidProfile(_))),
            "{bad:?}"
        );
    }
    if let Ok(named) = ProfileDirs::named("work_2") {
        assert!(named.root().ends_with("profiles/work_2"));
    }
}

#[test]
fn ensure_creates_a_private_directory() {
    let dir = scratch("ensure");
    let profile = ProfileDirs::at(dir.join("profiles/default"));
    profile.ensure().unwrap();
    profile.ensure().unwrap();
    assert!(profile.root().is_dir());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(profile.root())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700);
    }
    std::fs::remove_dir_all(dir).unwrap();
}
