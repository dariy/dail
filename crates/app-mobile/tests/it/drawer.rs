//! The drawer: modal over the list on phones and small tablets, permanent on wide windows (a
//! rail after `toggle_sidebar`); *All inboxes*, the accounts' colour tags, *Settings* and *Add
//! account*.

use app_core::{Command, Pane, ProviderPreset, SettingsSection};
use app_mobile::UiEvent;
use app_mobile::frame::PaneKind;

use crate::support::{Phone, inbox_of};

/// The inbox with a second account added (its mailboxes arrive with its first sync).
fn two_accounts(width_dp: u32) -> Phone {
    let mut phone = Phone::inbox(width_dp, 800);
    phone.dispatch(Command::AddAccount {
        preset: ProviderPreset::Gmail,
        email: "bob@gmail.com".to_owned(),
        display_name: Some("Bob".to_owned()),
        password: None,
        imap: None,
        smtp: None,
    });
    phone
}

#[test]
fn the_drawer_is_modal_over_the_list_on_small_windows() {
    for width in [360, 700] {
        let mut phone = Phone::inbox(width, 800);
        phone.dispatch(Command::FocusPane {
            pane: Pane::Sidebar,
        });
        let frame = phone.frame();
        let drawer = frame.drawer.unwrap();
        assert!(drawer.modal);
        assert!(!frame.layout.panes.contains(&PaneKind::Drawer));
        assert_eq!(frame.layout.panes[0], PaneKind::List);
        assert!(frame.list.is_some(), "the list stays under the drawer");
        assert!(frame.back, "Back closes the drawer");
    }
}

#[test]
fn a_tap_in_a_modal_drawer_opens_the_mailbox_and_closes_the_drawer() {
    let mut phone = Phone::inbox(360, 800);
    phone.dispatch(Command::FocusPane {
        pane: Pane::Sidebar,
    });
    let drawer = phone.frame().drawer.unwrap();
    let sent = drawer.accounts[0]
        .mailboxes
        .iter()
        .find(|m| m.icon == "send")
        .unwrap();
    let commands = phone.tap(sent.action);
    assert!(matches!(commands[0], Command::OpenMailbox { .. }));
    assert_eq!(commands[1], Command::FocusPane { pane: Pane::List });
    let frame = phone.frame();
    assert!(frame.drawer.is_none());
    assert_eq!(frame.list.unwrap().title, "Sent");
}

#[test]
fn back_closes_the_modal_drawer_before_the_thread() {
    let mut phone = Phone::inbox(700, 800);
    let (_, threads) = inbox_of(phone.state());
    phone.dispatch(Command::OpenThread { id: threads[0] });
    phone.dispatch(Command::FocusPane {
        pane: Pane::Sidebar,
    });
    phone.event(UiEvent::Back);
    assert_eq!(phone.state().view.focus, Pane::List);
    assert_eq!(phone.state().view.thread, Some(threads[0]));
}

#[test]
fn the_drawer_is_permanent_on_wide_windows() {
    // Portrait: in landscape it is a rail (MISC-6).
    let mut phone = Phone::inbox(900, 1280);
    let frame = phone.frame();
    let drawer = frame.drawer.unwrap();
    assert!(!drawer.modal && !drawer.rail);
    assert_eq!(frame.layout.panes[0], PaneKind::Drawer);
    assert!(frame.top_bar.navigation.is_none());
    // A tap only opens the mailbox: the drawer stays.
    let sent = drawer.accounts[0]
        .mailboxes
        .iter()
        .find(|m| m.icon == "send")
        .unwrap();
    assert_eq!(phone.tap(sent.action).len(), 1);
}

#[test]
fn one_account_has_no_tag_and_no_all_inboxes() {
    let mut phone = Phone::inbox(1280, 800);
    let drawer = phone.frame().drawer.unwrap();
    assert!(drawer.all_inboxes.is_none());
    assert_eq!(drawer.accounts[0].tag, None);
}

#[test]
fn two_accounts_have_tags_and_all_inboxes() {
    let mut phone = two_accounts(1280);
    let drawer = phone.frame().drawer.unwrap();
    let tags: Vec<_> = drawer.accounts.iter().map(|a| a.tag).collect();
    assert_eq!(tags, [Some(0), Some(1)]);
    let all = drawer.all_inboxes.unwrap();
    assert_eq!(all.icon, "all_inbox");
    assert!(!all.selected);
    assert_eq!(phone.tap(all.action), [Command::OpenUnifiedInbox {}]);
    let frame = phone.frame();
    assert!(frame.drawer.unwrap().all_inboxes.unwrap().selected);
    assert_eq!(frame.list.unwrap().title, "All inboxes");
}

#[test]
fn settings_and_add_account_open_the_settings_page() {
    let mut phone = Phone::inbox(360, 800);
    phone.dispatch(Command::FocusPane {
        pane: Pane::Sidebar,
    });
    let footer = phone.frame().drawer.unwrap().footer;
    let labels: Vec<_> = footer.iter().map(|i| i.label.as_str()).collect();
    assert_eq!(labels, ["Settings", "About", "Add account"]);
    let commands = phone.tap(footer[2].action);
    assert_eq!(
        commands[0],
        Command::OpenSettings {
            section: Some(SettingsSection::Accounts)
        }
    );
    let frame = phone.frame();
    assert_eq!(frame.layout.panes, [PaneKind::Settings]);
    assert_eq!(frame.settings.unwrap().section, "accounts");
    assert_eq!(frame.top_bar.title, "Settings");
    assert!(frame.fab.is_none());
    assert!(frame.back);
    phone.event(UiEvent::Back);
    assert_eq!(phone.state().view.settings, None);
}

#[test]
fn settings_keep_the_drawer_on_wide_windows() {
    let mut phone = Phone::inbox(1280, 800);
    phone.dispatch(Command::OpenSettings { section: None });
    let frame = phone.frame();
    assert_eq!(frame.layout.panes, [PaneKind::Drawer, PaneKind::Settings]);
    assert!(frame.drawer.unwrap().footer[0].selected);
}
