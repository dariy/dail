//! The settings page and the account setup: tabs, choices that run the settings commands, the
//! *Background & notifications* group, and setup from the first frame to *Sync now* (the
//! password never in a frame).

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use app_core::{AccountStatus, Command, Event, ProviderPreset, SettingKey, SettingsSection};
use app_mobile::event::{Field, SystemScreen};
use app_mobile::frame::{InputKind, SettingsPage, SetupSheet};
use app_mobile::{External, UiEvent};

use crate::support::Phone;

fn page(phone: &mut Phone, section: SettingsSection) -> SettingsPage {
    phone.dispatch(Command::OpenSettings {
        section: Some(section),
    });
    phone.frame().settings.expect("the settings page")
}

/// The action of the choice `label` in the row `row`.
fn choice(page: &SettingsPage, row: &str, label: &str) -> u32 {
    page.groups
        .iter()
        .flat_map(|g| &g.rows)
        .find(|r| r.label == row)
        .unwrap_or_else(|| panic!("no row {row}"))
        .choices
        .iter()
        .find(|c| c.label == label)
        .unwrap_or_else(|| panic!("no choice {label} in {row}"))
        .action
}

fn button(page: &SettingsPage, label: &str) -> u32 {
    page.groups
        .iter()
        .flat_map(|g| &g.rows)
        .flat_map(|r| &r.actions)
        .find(|b| b.label == label)
        .unwrap_or_else(|| panic!("no button {label}"))
        .action
}

fn sheet_button(sheet: &SetupSheet, label: &str) -> u32 {
    sheet
        .actions
        .iter()
        .find(|b| b.label == label)
        .unwrap_or_else(|| panic!("no {label} in {:?}", sheet.actions))
        .action
}

#[test]
fn the_tabs_open_the_sections() {
    let mut phone = Phone::inbox(360, 800);
    let page = page(&mut phone, SettingsSection::Appearance);
    let tabs: Vec<_> = page.tabs.iter().map(|t| t.label.as_str()).collect();
    assert_eq!(
        tabs,
        ["Appearance", "Keyboard", "General", "Accounts", "Rules"]
    );
    assert!(page.tabs[0].selected);
    assert_eq!(
        phone.tap(page.tabs[2].action),
        [Command::OpenSettings {
            section: Some(SettingsSection::General)
        }]
    );
    assert_eq!(phone.frame().settings.unwrap().section, "general");
}

#[test]
fn choices_run_the_settings_commands() {
    let mut phone = Phone::inbox(360, 800);
    let general = page(&mut phone, SettingsSection::General);
    assert_eq!(
        phone.tap(choice(&general, "Undo send", "30 s")),
        [Command::SetSetting {
            key: SettingKey::UndoSendMs,
            value: 30_000u64.into()
        }]
    );
    assert_eq!(phone.state().settings.undo_send_ms, 30_000);
    let general = phone.frame().settings.unwrap();
    let undo = general.groups[0]
        .rows
        .iter()
        .find(|r| r.label == "Undo send")
        .unwrap();
    assert_eq!(undo.value, "30 s");
    assert!(undo.choices.iter().any(|c| c.label == "30 s" && c.selected));

    let appearance = page(&mut phone, SettingsSection::Appearance);
    assert_eq!(
        phone.tap(choice(&appearance, "Mode", "Dark")),
        [Command::SetTheme {
            mode: Some(app_core::ThemeMode::Dark),
            light: None,
            dark: None
        }]
    );
    assert!(
        phone.frame().theme.is_some(),
        "the new theme goes to the host"
    );
    let appearance = phone.frame().settings.unwrap();
    phone.tap(choice(&appearance, "List density", "Compact"));
    assert_eq!(phone.state().settings.list_density, 1);

    let keys = page(&mut phone, SettingsSection::Keys);
    assert_eq!(
        phone.tap(choice(&keys, "Keys", "Vim")),
        [Command::SetKeyPreset {
            preset: app_core::KeyPreset::Vim
        }]
    );
    assert!(keys.groups[1].rows.iter().any(|r| r.label == "Archive"));
}

#[test]
fn background_and_notifications_live_under_general() {
    let mut phone = Phone::inbox(360, 800);
    let general = page(&mut phone, SettingsSection::General);
    let group = general
        .groups
        .iter()
        .find(|g| g.title == "Background & notifications")
        .unwrap();
    let labels: Vec<_> = group.rows.iter().map(|r| r.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Mail checks in the background",
            "Check every",
            "Notifications",
            "System settings"
        ]
    );
    phone.tap(choice(&general, "Mail checks in the background", "Push"));
    assert_eq!(phone.state().settings.background_sync, 2);
    let general = phone.frame().settings.unwrap();
    phone.tap(choice(&general, "Check every", "1 h"));
    assert_eq!(phone.state().settings.background_interval_mins, 60);
    phone.externals();
    let general = phone.frame().settings.unwrap();
    phone.tap(button(&general, "Battery optimization"));
    assert_eq!(
        phone.externals(),
        [External::SystemSettings {
            screen: SystemScreen::Battery
        }]
    );
    // Only while open: no interval to choose.
    let general = phone.frame().settings.unwrap();
    phone.tap(choice(
        &general,
        "Mail checks in the background",
        "Only while open",
    ));
    let general = phone.frame().settings.unwrap();
    assert!(
        !general
            .groups
            .iter()
            .flat_map(|g| &g.rows)
            .any(|r| r.label == "Check every")
    );
}

#[test]
fn accounts_can_be_tested_and_removed_after_a_confirmation() {
    let mut phone = Phone::inbox(360, 800);
    let account = *phone.state().accounts.keys().next().unwrap();
    let accounts = page(&mut phone, SettingsSection::Accounts);
    assert_eq!(accounts.groups[0].rows[0].label, "alice@example.com");
    assert_eq!(
        phone.tap(button(&accounts, "Test")),
        [Command::TestAccount { id: account }]
    );
    let accounts = phone.frame().settings.unwrap();
    assert_eq!(
        phone.tap(button(&accounts, "Remove")),
        [Command::OpenPalette {}]
    );
    let sheet = phone.frame().palette.unwrap();
    assert!(sheet.prompt.unwrap().starts_with("Remove this account?"));
    let yes = sheet.sections[0].rows[0].action;
    let got = phone.tap(yes);
    assert_eq!(
        got[0],
        Command::RemoveAccount {
            id: account,
            confirm: true
        }
    );
    assert!(phone.state().accounts.is_empty());
}

#[test]
fn setup_adds_tests_and_syncs_an_account_without_showing_the_password() {
    let mut phone = Phone::new(app_core::AppState::default(), 360, 800);
    // A profile without accounts opens the setup once.
    let frame = phone.frame();
    assert!(frame.back);
    let sheet = frame.setup.expect("the setup opens on its own");
    assert_eq!(sheet.step, "1 / 4  Provider");
    let custom = sheet
        .presets
        .iter()
        .find(|p| p.label == "Other IMAP / SMTP")
        .unwrap()
        .action;
    phone.tap(custom);
    let sheet = phone.frame().setup.unwrap();
    phone.tap(sheet_button(&sheet, "Next"));
    let sheet = phone.frame().setup.unwrap();
    assert_eq!(sheet.step, "2 / 4  Address");
    let kinds: Vec<_> = sheet.fields.iter().map(|f| (f.field, f.input)).collect();
    assert_eq!(
        kinds,
        [
            (Field::Email, InputKind::Email),
            (Field::Name, InputKind::Name),
            (Field::Imap, InputKind::Server),
            (Field::Smtp, InputKind::Server),
        ]
    );
    assert!(sheet.actions.is_empty(), "nothing to go on with yet");
    phone.event(UiEvent::Text {
        field: Field::Email,
        text: "me@example.org".to_owned(),
    });
    let sheet = phone.frame().setup.unwrap();
    assert!(sheet.error.as_deref().unwrap().starts_with("IMAP"));
    assert_eq!(
        phone.tap(sheet_button(&sheet, "Find servers")),
        [Command::DiscoverServers {
            email: "me@example.org".to_owned()
        }]
    );
    let version = sheet.fields[2].value.version;
    phone.ingest(Event::ServersDiscovered {
        email: "me@example.org".to_owned(),
        imap: app_core::Endpoint::parse("imap.example.org:993").unwrap(),
        smtp: app_core::Endpoint::parse("smtp.example.org:465").unwrap(),
        source: "autoconfig".to_owned(),
    });
    let sheet = phone.frame().setup.unwrap();
    assert_eq!(sheet.fields[2].value.text, "imap.example.org:993:tls");
    assert!(
        sheet.fields[2].value.version > version,
        "the host reloads the field"
    );
    phone.tap(sheet_button(&sheet, "Next"));
    let sheet = phone.frame().setup.unwrap();
    assert_eq!(sheet.fields[0].input, InputKind::Password);
    phone.event(UiEvent::Text {
        field: Field::Password,
        text: "hunter2-secret".to_owned(),
    });
    let frame = phone.frame();
    assert!(!serde_json::to_string(&frame).unwrap().contains("hunter2"));
    let sheet = frame.setup.unwrap();
    let got = phone.tap(sheet_button(&sheet, "Add and test"));
    assert!(
        matches!(&got[0], Command::AddAccount { preset: ProviderPreset::Custom, email, .. } if email == "me@example.org")
    );
    let id = *phone.state().accounts.keys().next().unwrap();
    assert_eq!(got[1], Command::TestAccount { id });
    let sheet = phone.frame().setup.unwrap();
    assert!(sheet.busy);
    assert!(sheet.fields.is_empty());
    phone.ingest(Event::AccountVerified { account: id });
    let sheet = phone.frame().setup.unwrap();
    assert_eq!(
        phone.tap(sheet_button(&sheet, "Sync now")),
        [Command::SyncNow { account: Some(id) }]
    );
    assert!(phone.frame().setup.is_none());
    // Once closed, it does not open again on its own.
    assert!(phone.frame().setup.is_none());
}

/// STORE-12: a Microsoft address stops at the address step with a clear reason; Gmail's password step links to
/// Google's App Password page.
#[test]
fn oauth_only_addresses_stop_early_and_gmail_links_to_app_passwords() {
    let mut phone = Phone::new(app_core::AppState::default(), 360, 800);
    let sheet = phone.frame().setup.unwrap();
    let custom = sheet
        .presets
        .iter()
        .find(|p| p.label == "Other IMAP / SMTP")
        .unwrap()
        .action;
    phone.tap(custom);
    let sheet = phone.frame().setup.unwrap();
    phone.tap(sheet_button(&sheet, "Next"));
    phone.event(UiEvent::Text {
        field: Field::Email,
        text: "someone@Hotmail.co.uk".to_owned(),
    });
    let sheet = phone.frame().setup.unwrap();
    let error = sheet.error.unwrap();
    assert!(
        error.starts_with("Microsoft addresses need a sign-in"),
        "{error}"
    );
    assert!(sheet.actions.iter().all(|a| a.label != "Next"));

    // Back to the providers, then Gmail.
    phone.event(UiEvent::Back);
    let sheet = phone.frame().setup.unwrap();
    assert_eq!(sheet.step, "1 / 4  Provider");
    let gmail = sheet
        .presets
        .iter()
        .find(|p| p.label == "Gmail")
        .unwrap()
        .action;
    phone.tap(gmail);
    let sheet = phone.frame().setup.unwrap();
    phone.tap(sheet_button(&sheet, "Next"));
    phone.event(UiEvent::Text {
        field: Field::Email,
        text: "me@gmail.com".to_owned(),
    });
    let sheet = phone.frame().setup.unwrap();
    phone.tap(sheet_button(&sheet, "Next"));
    let sheet = phone.frame().setup.unwrap();
    assert_eq!(sheet.fields[0].input, InputKind::Password);
    let _ = phone.externals();
    phone.tap(sheet_button(&sheet, "Create an App Password"));
    assert_eq!(
        phone.externals(),
        [External::OpenUrl {
            url: "https://myaccount.google.com/apppasswords".to_owned()
        }]
    );
    assert!(phone.frame().setup.is_some(), "the setup stays open");
}

#[test]
fn a_failed_login_tries_another_password_and_the_banner_reenters_it() {
    let mut phone = Phone::new(app_core::AppState::default(), 360, 800);
    let sheet = phone.frame().setup.unwrap();
    phone.tap(sheet_button(&sheet, "Next"));
    phone.event(UiEvent::Text {
        field: Field::Email,
        text: "me@gmail.com".to_owned(),
    });
    let sheet = phone.frame().setup.unwrap();
    assert_eq!(sheet.fields.len(), 2, "Gmail needs no servers");
    phone.tap(sheet_button(&sheet, "Next"));
    phone.event(UiEvent::Text {
        field: Field::Password,
        text: "wrong".to_owned(),
    });
    let sheet = phone.frame().setup.unwrap();
    assert_eq!(sheet.fields[0].label, "App Password");
    phone.tap(sheet_button(&sheet, "Add and test"));
    let id = *phone.state().accounts.keys().next().unwrap();
    phone.ingest(Event::AccountVerifyFailed {
        account: id,
        error: "IMAP LOGIN failed: [AUTHENTICATIONFAILED]".to_owned(),
    });
    let sheet = phone.frame().setup.unwrap();
    assert!(
        sheet
            .error
            .as_deref()
            .unwrap()
            .contains("AUTHENTICATIONFAILED")
    );
    assert_eq!(
        phone.tap(sheet_button(&sheet, "Try another password")),
        [Command::RemoveAccount { id, confirm: true }]
    );
    let sheet = phone.frame().setup.unwrap();
    assert_eq!(sheet.step, "3 / 4  Password");
    let labels: Vec<&str> = sheet.actions.iter().map(|a| a.label.as_str()).collect();
    // Only Gmail's link (STORE-12): no submit without a password.
    assert_eq!(
        labels,
        ["Create an App Password"],
        "the old password is gone"
    );
    // Back steps back, then closes.
    phone.event(UiEvent::Back);
    phone.event(UiEvent::Back);
    phone.event(UiEvent::Back);
    assert!(phone.frame().setup.is_none());

    // An existing account whose login fails: the banner's fix is the password form.
    let mut phone = Phone::inbox(360, 800);
    let id = *phone.state().accounts.keys().next().unwrap();
    phone.ingest(Event::AccountVerifyFailed {
        account: id,
        error: "IMAP LOGIN failed: [AUTHENTICATIONFAILED] Invalid credentials".to_owned(),
    });
    assert!(matches!(
        phone.state().accounts[&id].status,
        AccountStatus::Failed { .. }
    ));
    let banner = phone.frame().banner.unwrap();
    phone.tap(banner.action.expect("a fix").action);
    let sheet = phone.frame().setup.expect("the password form");
    assert_eq!(sheet.title, "Change the password");
    assert_eq!(sheet.step, "alice@example.com");
    phone.event(UiEvent::Text {
        field: Field::Password,
        text: "right".to_owned(),
    });
    let sheet = phone.frame().setup.unwrap();
    let got = phone.tap(sheet_button(&sheet, "Save and test"));
    assert!(matches!(got[0], Command::SetPassword { id: got_id, .. } if got_id == id));
}

/// A rule that stars mail from `news@` (SET-5).
pub fn news_rule() -> Command {
    Command::SaveRule {
        id: None,
        rule: app_core::Rule {
            name: "News".into(),
            enabled: true,
            when: app_core::Condition::From {
                contains: "news@".into(),
            },
            then: vec![app_core::RuleAction::Star],
        },
    }
}

/// SET-5: the Rules section lists the rules with on/off, *Edit…* and *Delete*, adds one through
/// the palette's steps, and stores the edits on the server.
#[test]
fn the_rules_section_drives_the_rule_commands() {
    let mut phone = Phone::inbox(360, 800);
    phone.dispatch(news_rule());
    let id = *phone.state().synced.settings.rules.keys().next().unwrap();
    let rules = page(&mut phone, SettingsSection::Rules);
    assert_eq!(rules.section, "rules");
    let row = rules.groups[0]
        .rows
        .iter()
        .find(|r| r.label == "News")
        .unwrap();
    assert!(row.hint.as_deref().unwrap().contains("news@"), "{row:?}");
    let off = choice(&rules, "News", "Off");
    assert_eq!(phone.tap(off), [Command::EnableRule { id, enabled: false }]);
    let rules = page(&mut phone, SettingsSection::Rules);
    let delete = button(&rules, "Delete");
    assert_eq!(phone.tap(delete), [Command::DeleteRule { id }]);
    let rules = page(&mut phone, SettingsSection::Rules);
    let store = button(&rules, "Store on the server");
    assert_eq!(phone.tap(store), [Command::PushSettings {}]);
    let rules = page(&mut phone, SettingsSection::Rules);
    let add = button(&rules, "Add a rule…");
    assert_eq!(phone.tap(add), [Command::OpenPalette {}]);
    let sheet = phone
        .frame()
        .palette
        .expect("the palette at the first step");
    assert_eq!(sheet.prompt.as_deref(), Some("Match mail by what?"));
}
