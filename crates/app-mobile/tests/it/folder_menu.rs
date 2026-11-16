//! TCH-4: a long press on a folder in the drawer opens its actions (`mailbox_actions`) in the
//! action sheet; a tap runs an entry or opens its palette step, Back and *Close* dismiss it.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::{Command, Pane};
use app_front::i18n;
use app_mobile::UiEvent;
use app_mobile::event::Field;
use app_mobile::frame::ActionSheet;

use crate::support::{
    Phone, SIZES, folders_over, mailbox, open_menu, requests_of, synced_state, with_folders,
};

fn item(sheet: &ActionSheet, label: &str) -> u32 {
    sheet
        .items
        .iter()
        .find(|b| b.label == label)
        .unwrap_or_else(|| panic!("{label} in {:?}", sheet.items))
        .action
}

/// The commands of a tap, without the modal drawer's `focus_pane list`.
fn tap(phone: &mut Phone, action: u32) -> Vec<Command> {
    phone
        .tap(action)
        .into_iter()
        .filter(|c| !matches!(c, Command::FocusPane { .. }))
        .collect()
}

#[test]
fn only_real_mailboxes_have_a_long_press() {
    for (w, h) in SIZES {
        let mut phone = with_folders(w, h);
        // A second account shows *All inboxes*.
        phone.dispatch(Command::AddAccount {
            preset: app_core::ProviderPreset::Gmail,
            email: "bob@gmail.com".to_owned(),
            display_name: Some("Bob".to_owned()),
            password: None,
            imap: None,
            smtp: None,
        });
        let drawer = phone.frame().drawer.unwrap();
        assert!(drawer.all_inboxes.unwrap().long_press.is_none());
        assert!(drawer.footer.iter().all(|f| f.long_press.is_none()));
        let folders = &drawer.accounts[0].mailboxes;
        assert!(folders.len() >= 7);
        assert!(folders.iter().all(|f| f.long_press.is_some()), "{w}×{h}");
    }
}

#[test]
fn the_sheet_lists_the_folder_actions_in_order() {
    for (w, h) in SIZES {
        for path in ["INBOX", "Trash", "Junk", "Work"] {
            let mut phone = with_folders(w, h);
            let sheet = open_menu(&mut phone, path);
            let id = mailbox(&phone, path);
            let state = phone.state();
            assert_eq!(
                sheet.title,
                app_front::mailbox::mailbox_name(&state.mailboxes[&id])
            );
            let expected: Vec<(Option<String>, String)> =
                app_front::actions::mailbox_actions(state, id)
                    .into_iter()
                    .map(|e| (Some(e.icon.to_owned()), i18n::s(e.label).to_owned()))
                    .collect();
            let got: Vec<(Option<String>, String)> = sheet
                .items
                .iter()
                .map(|b| (b.icon.clone(), b.label.clone()))
                .collect();
            assert_eq!(got, expected, "{path} at {w}×{h}");
            assert_eq!(sheet.close.label, "Close");
        }
    }
}

#[test]
fn pin_runs_at_once_and_closes_the_sheet() {
    for (w, h) in SIZES {
        let mut phone = with_folders(w, h);
        let id = mailbox(&phone, "Work");
        let sheet = open_menu(&mut phone, "Work");
        assert_eq!(
            tap(&mut phone, item(&sheet, "Pin to Favourites")),
            [Command::PinMailbox { id }]
        );
        assert!(phone.frame().menu.is_none(), "{w}×{h}");
    }
}

#[test]
fn a_modal_drawer_closes_when_an_entry_is_chosen() {
    let mut phone = with_folders(360, 800);
    let id = mailbox(&phone, "Work");
    open_menu(&mut phone, "Work");
    let frame = phone.frame();
    assert!(
        frame.drawer.unwrap().modal,
        "the drawer stays under the sheet"
    );
    let sheet = frame.menu.unwrap();
    let got = phone.tap(item(&sheet, "Focus this mailbox"));
    assert_eq!(
        got,
        [
            Command::FocusPane { pane: Pane::List },
            Command::FocusMailbox {
                id: Some(id),
                account: None
            }
        ]
    );
    let frame = phone.frame();
    assert!(frame.drawer.is_none() && frame.menu.is_none());
}

/// Types `text` into the palette's text step and takes its first row.
fn answer(phone: &mut Phone, text: &str) -> Vec<Command> {
    phone.event(UiEvent::Text {
        field: Field::Query,
        text: text.to_owned(),
    });
    let frame = phone.frame();
    let first = frame.palette.unwrap().sections[0].rows[0].action;
    requests_of(&{
        phone.take();
        phone.event(UiEvent::Tap { action: first });
        phone.take()
    })
}

#[test]
fn rename_and_new_subfolder_ask_for_a_name() {
    for (w, h) in SIZES {
        let mut phone = with_folders(w, h);
        let id = mailbox(&phone, "Work");
        let account = phone.state().mailboxes[&id].account;
        let sheet = open_menu(&mut phone, "Work");
        assert_eq!(
            tap(&mut phone, item(&sheet, "Rename…")),
            [Command::OpenPalette {}]
        );
        let frame = phone.frame();
        assert!(frame.menu.is_none());
        assert!(frame.palette.unwrap().prompt.is_some(), "a text step");
        let got = answer(&mut phone, "Jobs");
        assert_eq!(
            got[0],
            Command::RenameFolder {
                id,
                path: "Jobs".into()
            },
            "{w}×{h}"
        );

        let mut phone = with_folders(w, h);
        let sheet = open_menu(&mut phone, "Work");
        assert_eq!(
            tap(&mut phone, item(&sheet, "New subfolder…")),
            [Command::OpenPalette {}]
        );
        let got = answer(&mut phone, "Clients");
        assert_eq!(
            got[0],
            Command::CreateFolder {
                account,
                path: "Work/Clients".into()
            },
            "{w}×{h}"
        );
    }
}

/// Taps the entry `label` of `path`'s sheet; it must open a confirmation, whose *yes* is taken.
fn confirmed(phone: &mut Phone, path: &str, label: &str) -> Vec<Command> {
    let sheet = open_menu(phone, path);
    assert_eq!(tap(phone, item(&sheet, label)), [Command::OpenPalette {}]);
    let frame = phone.frame();
    let palette = frame.palette.unwrap();
    assert!(palette.prompt.is_some(), "{label} asks first");
    phone.tap(palette.sections[0].rows[0].action)
}

#[test]
fn delete_empty_and_a_large_mark_all_read_confirm() {
    for (w, h) in SIZES {
        let mut phone = with_folders(w, h);
        let id = mailbox(&phone, "Work");
        assert_eq!(
            confirmed(&mut phone, "Work", "Delete…")[0],
            Command::DeleteFolder { id, confirm: true }
        );

        let mut phone = with_folders(w, h);
        let account = *phone.state().accounts.keys().next().unwrap();
        assert_eq!(
            confirmed(&mut phone, "Trash", "Empty Trash")[0],
            Command::EmptyTrash {
                account,
                confirm: true
            }
        );
        let mut phone = with_folders(w, h);
        assert_eq!(
            confirmed(&mut phone, "Junk", "Empty Junk")[0],
            Command::EmptyJunk {
                account,
                confirm: true
            }
        );

        // Few unread: at once.
        let mut phone = with_folders(w, h);
        let inbox = mailbox(&phone, "INBOX");
        let sheet = open_menu(&mut phone, "INBOX");
        assert_eq!(
            tap(&mut phone, item(&sheet, "Mark all as read")),
            [Command::MarkMailboxRead { id: inbox }]
        );
        // Many: asks first.
        let mut state = synced_state();
        for mailbox in state.mailboxes.values_mut() {
            mailbox.unread = 80;
        }
        let mut phone = folders_over(state, w, h);
        assert_eq!(
            confirmed(&mut phone, "INBOX", "Mark all as read")[0],
            Command::MarkMailboxRead { id: inbox }
        );
    }
}

#[test]
fn back_and_close_dismiss_without_commands() {
    for (w, h) in SIZES {
        let mut phone = with_folders(w, h);
        open_menu(&mut phone, "Work");
        phone.take();
        phone.event(UiEvent::Back);
        assert!(phone.take().is_empty(), "{w}×{h}");
        let frame = phone.frame();
        assert!(frame.menu.is_none());
        assert!(frame.drawer.is_some(), "Back closed only the sheet");

        let sheet = open_menu(&mut phone, "Work");
        assert!(phone.tap(sheet.close.action).is_empty());
        assert!(phone.frame().menu.is_none());
    }
}

#[test]
fn the_sheet_closes_when_its_folder_is_gone() {
    for (w, h) in SIZES {
        let mut phone = with_folders(w, h);
        let id = mailbox(&phone, "Work");
        open_menu(&mut phone, "Work");
        phone.dispatch(Command::DeleteFolder { id, confirm: true });
        assert!(
            !phone.state().mailboxes.contains_key(&id),
            "deleted locally"
        );
        let frame = phone.frame();
        assert!(frame.menu.is_none(), "{w}×{h}");
        assert!(phone.frame().menu.is_none(), "and stays closed");
    }
}
