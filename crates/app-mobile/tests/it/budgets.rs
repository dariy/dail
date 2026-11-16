//! QA-2: the frame budgets of the phone, stated once as rules over every touch state at
//! 360×640 dp (a small phone, the tightest window the presenter targets):
//!
//! * **A** at most four icons per bar, and every button of a bar has a label (TalkBack's name);
//! * **B** a palette entry in every state that shows a bar, exactly one;
//! * **C** no settings section more than one tap from the settings root.
//!
//! A change that breaks a budget fails here with the state and the bar named.
//!
//! Which lists are bars: `top_bar`, `bulk` and `reader` (each `actions` beside an `overflow`)
//! and the composer's `actions`. They are the rows of icons along the top of the screen, and
//! the presenter splits them by the window class (TCH-2). Not bars: a message card's `actions`
//! (a menu of the message), a setup sheet's `actions`, a settings row's `actions`, the action
//! sheet's `items`, the recipients' `suggestions` and the reader's `links` (menus, sheets and
//! rows of their own, which scroll or sit in a sheet), nor the single buttons `navigation`,
//! `close`, `maximize` and `fab`, which ARCHITECTURE says do not count against a budget.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::{Command, MailboxRole, SettingsSection};
use app_mobile::frame::{Button, PaneKind};
use app_mobile::{Frame, UiEvent};

use crate::support::{
    Phone, bar_buttons, composing, configure, folders_over, from_drawer, inbox_of, mailbox,
    maximized, open_first, open_menu, synced_state,
};

const W: u32 = 360;
const H: u32 = 640;

/// One touch state: how to reach it from a fresh phone, and why it has no palette entry, if so.
struct State {
    name: &'static str,
    build: fn() -> Phone,
    /// Budget B's exemption: the reason the state shows no palette button.
    exempt: Option<&'static str>,
}

fn inbox() -> Phone {
    Phone::inbox(W, H)
}

fn searching() -> Phone {
    let mut phone = inbox();
    phone.dispatch(Command::SetSearch {
        query: "lunch".to_owned(),
    });
    phone
}

fn message() -> Phone {
    let mut phone = inbox();
    open_first(&mut phone);
    phone
}

/// An unread, starred thread open (the read toggle and the star both change their labels).
fn starred_unread() -> Phone {
    let mut phone = inbox();
    let (_, threads) = inbox_of(phone.state());
    phone.dispatch(Command::MarkUnread {
        ids: vec![threads[0]],
        messages: Vec::new(),
    });
    phone.dispatch(Command::Star {
        ids: vec![threads[0]],
        messages: Vec::new(),
    });
    phone.dispatch(Command::OpenThread { id: threads[0] });
    phone
}

fn maximized_message() -> Phone {
    let mut phone = inbox();
    maximized(&mut phone);
    phone
}

fn bulk_one() -> Phone {
    let mut phone = inbox();
    let (_, threads) = inbox_of(phone.state());
    phone.dispatch(Command::CheckThreads {
        ids: vec![threads[0]],
    });
    phone
}

/// Every shown thread checked, by the bar's own *Select all*.
fn bulk_all() -> Phone {
    let mut phone = bulk_one();
    let bulk = phone.frame().bulk.unwrap();
    let select = bulk
        .actions
        .iter()
        .chain(&bulk.overflow)
        .find(|b| b.label == "Select all")
        .unwrap()
        .action;
    phone.tap(select);
    phone
}

/// A thread checked in the Junk folder, where *Not junk* replaces *Junk*.
fn bulk_in_junk() -> Phone {
    let mut state = synced_state();
    state.settings.stale_secs = 86_400;
    let (inbox, threads) = inbox_of(&state);
    let mut folder = state.mailboxes[&inbox].clone();
    folder.id = app_core::IdGen::new(78).next_id();
    folder.path = "Junk".into();
    folder.role = Some(MailboxRole::Junk);
    let junk = folder.id;
    state.mailboxes.insert(junk, folder);
    let mut phone = Phone::new(state, W, H);
    // The thread is in Junk: only a thread the list shows stays checked.
    phone.dispatch(Command::MarkJunk {
        ids: vec![threads[0]],
        messages: Vec::new(),
    });
    phone.dispatch(Command::OpenMailbox { id: junk });
    phone.dispatch(Command::CheckThreads {
        ids: vec![threads[0]],
    });
    phone
}

fn composer_new() -> Phone {
    let mut phone = inbox();
    composing(&mut phone);
    phone
}

fn composer_reply() -> Phone {
    let mut phone = message();
    let reply = phone
        .frame()
        .reader
        .unwrap()
        .actions
        .into_iter()
        .find(|b| b.label == "Reply")
        .unwrap();
    phone.tap(reply.action);
    phone
}

fn drawer() -> Phone {
    let mut phone = inbox();
    phone.dispatch(Command::FocusPane {
        pane: app_core::Pane::Sidebar,
    });
    phone
}

fn folder_sheet() -> Phone {
    let mut phone = folders_over(
        {
            let mut state = synced_state();
            state.settings.stale_secs = 86_400;
            state
        },
        W,
        H,
    );
    open_menu(&mut phone, "Work");
    assert!(mailbox(&phone, "Work") != mailbox(&phone, "INBOX"));
    phone
}

fn root(keyboard: bool) -> Phone {
    let mut phone = inbox();
    configure(&mut phone, W, H, keyboard);
    from_drawer(&mut phone);
    phone
}

fn root_with_keyboard() -> Phone {
    root(true)
}

fn root_without_keyboard() -> Phone {
    root(false)
}

fn section(section: SettingsSection) -> Phone {
    let mut phone = inbox();
    phone.dispatch(Command::OpenSettings {
        section: Some(section),
    });
    phone
}

fn accounts() -> Phone {
    section(SettingsSection::Accounts)
}
fn rules() -> Phone {
    section(SettingsSection::Rules)
}
fn appearance() -> Phone {
    section(SettingsSection::Appearance)
}
fn general() -> Phone {
    section(SettingsSection::General)
}
fn notifications() -> Phone {
    section(SettingsSection::Notifications)
}
fn keyboard() -> Phone {
    let mut phone = root(true);
    let entry = entry(&mut phone, "Keyboard");
    phone.tap(entry);
    phone
}

fn about_from_settings() -> Phone {
    let mut phone = root(true);
    let entry = entry(&mut phone, "About");
    phone.tap(entry);
    phone
}

fn palette() -> Phone {
    let mut phone = inbox();
    phone.dispatch(Command::OpenPalette {});
    phone
}

fn no_accounts() -> Phone {
    Phone::new(app_core::AppState::default(), W, H)
}

/// The action of the settings root's entry `label`.
fn entry(phone: &mut Phone, label: &str) -> u32 {
    phone
        .frame()
        .settings
        .unwrap()
        .sections
        .iter()
        .find(|e| e.label == label)
        .unwrap_or_else(|| panic!("{label} in the settings root"))
        .action
}

const ABOUT: &str = "the About screen covers the whole screen and Back closes it";

/// Every touch state of the phone.
fn states() -> Vec<State> {
    let s = |name, build, exempt| State {
        name,
        build,
        exempt,
    };
    vec![
        s("inbox", inbox, None),
        s("inbox, search open", searching, None),
        s("message", message, None),
        s("message, unread and starred", starred_unread, None),
        s("message, maximized", maximized_message, None),
        s("bulk, one thread", bulk_one, None),
        s("bulk, all threads", bulk_all, None),
        s("bulk, in Junk", bulk_in_junk, None),
        s("composer, new message", composer_new, None),
        s("composer, reply", composer_reply, None),
        s("drawer", drawer, None),
        s("folder action sheet", folder_sheet, None),
        s("settings root, keyboard", root_with_keyboard, None),
        s("settings root, no keyboard", root_without_keyboard, None),
        s("settings, Accounts", accounts, None),
        s("settings, Rules", rules, None),
        s("settings, Appearance", appearance, None),
        s("settings, General", general, None),
        s("settings, Notifications", notifications, None),
        s("settings, Keyboard", keyboard, None),
        s("About", about_from_settings, Some(ABOUT)),
        // Not exempt: the frame keeps the top bar (and its search) under the sheet.
        s("palette", palette, None),
        s(
            "no accounts",
            no_accounts,
            Some(
                "its top bar keeps Add account and About: the palette is no use without an account",
            ),
        ),
    ]
}

/// The lists that are bars in `frame`: `(bar, actions, overflow)`.
fn bars(frame: &Frame) -> Vec<(&'static str, &[Button], &[Button])> {
    let mut out: Vec<(&'static str, &[Button], &[Button])> =
        vec![("top_bar", &frame.top_bar.actions, &frame.top_bar.overflow)];
    if let Some(bulk) = &frame.bulk {
        out.push(("bulk", &bulk.actions, &bulk.overflow));
    }
    if let Some(reader) = &frame.reader {
        out.push(("reader", &reader.actions, &reader.overflow));
    }
    if let Some(composer) = &frame.composer {
        out.push(("composer", &composer.actions, &[]));
    }
    out
}

/// What each state's bar looks like, for a failure message.
fn describe(buttons: &[Button]) -> String {
    let labels: Vec<_> = buttons.iter().map(|b| b.label.as_str()).collect();
    format!("{labels:?}")
}

#[test]
fn the_table_reaches_the_states_it_names() {
    // A state that silently fell back to the inbox would make every budget below vacuous.
    for state in states() {
        let frame = (state.build)().frame();
        let reached = match state.name {
            "inbox, search open" => frame
                .list
                .as_ref()
                .is_some_and(|l| l.title.contains("lunch")),
            "message" | "message, unread and starred" => {
                frame.reader.is_some() && frame.layout.page == Some(PaneKind::Reader)
            }
            "message, maximized" => frame.reader.is_some(),
            n if n.starts_with("bulk") => frame.bulk.is_some(),
            n if n.starts_with("composer") => frame.composer.is_some(),
            "drawer" => frame.drawer.as_ref().is_some_and(|d| d.modal),
            "folder action sheet" => frame.menu.is_some(),
            "About" => frame.about.is_some(),
            "palette" => frame.palette.is_some(),
            "no accounts" => frame.list.is_none() || frame.top_bar.actions.len() > 1,
            n if n.starts_with("settings") => frame.settings.is_some(),
            _ => frame.list.is_some(),
        };
        assert!(reached, "the state {:?} is not reached", state.name);
    }
}

#[test]
fn budget_a_at_most_four_icons_per_bar_each_with_a_label() {
    for state in states() {
        let frame = (state.build)().frame();
        for (bar, actions, overflow) in bars(&frame) {
            assert!(
                actions.len() <= 4,
                "{}: the {bar} bar has {} icons: {}",
                state.name,
                actions.len(),
                describe(actions)
            );
            for button in actions.iter().chain(overflow) {
                assert!(
                    !button.label.trim().is_empty(),
                    "{}: a button of the {bar} bar (icon {:?}, action {}) has no label",
                    state.name,
                    button.icon,
                    button.action
                );
            }
        }
        for (what, button) in [
            ("navigation", frame.top_bar.navigation.as_ref()),
            ("fab", frame.fab.as_ref()),
            ("bulk close", frame.bulk.as_ref().map(|b| &b.close)),
            ("reader close", frame.reader.as_ref().map(|r| &r.close)),
            (
                "reader maximize",
                frame.reader.as_ref().and_then(|r| r.maximize.as_ref()),
            ),
        ] {
            if let Some(button) = button {
                assert!(
                    !button.label.trim().is_empty(),
                    "{}: the {what} button has no label",
                    state.name
                );
            }
        }
    }
}

#[test]
fn the_budget_leaves_out_what_architecture_says_it_leaves_out() {
    // The bars' icons are `actions`: close, maximize and the navigation button are fields of
    // their own, so they come on top of the four, and the menu trigger is no entry at all.
    let mut phone = bulk_one();
    let frame = phone.frame();
    let bulk = frame.bulk.unwrap();
    assert_eq!(bulk.actions.len(), 4, "a full bulk bar");
    assert!(
        bulk.actions
            .iter()
            .chain(&bulk.overflow)
            .all(|b| b.action != bulk.close.action)
    );
    assert!(frame.top_bar.navigation.is_some(), "its own field");

    let mut phone = message();
    let frame = phone.frame();
    let reader = frame.reader.unwrap();
    assert_eq!(reader.actions.len(), 4, "a full reader bar");
    for own in [Some(&reader.close), reader.maximize.as_ref()]
        .into_iter()
        .flatten()
    {
        assert!(
            reader
                .actions
                .iter()
                .chain(&reader.overflow)
                .all(|b| b.action != own.action),
            "{} is not in the list",
            own.label
        );
    }
    // Previous and next are not buttons of the bar but a pull on the body (`Neighbour`).
    let _: Option<&app_mobile::frame::Neighbour> = reader.previous.as_ref();
    let _: Option<&app_mobile::frame::Neighbour> = reader.next.as_ref();
    // The menu trigger is the host's: no entry of the lists is called *More actions*.
    for (_, actions, overflow) in bars(&phone.frame()) {
        assert!(
            actions
                .iter()
                .chain(overflow)
                .all(|b| b.label != "More actions")
        );
    }
}

/// How many bar buttons of the state open the palette at its top, and how many carry the
/// `search` icon. Each tap is made on a fresh copy of the state, as a tap changes it.
fn palette_entries(state: &State) -> (usize, usize) {
    let total = bar_buttons(&(state.build)().frame()).len();
    let (mut entries, mut searches) = (0, 0);
    for at in 0..total {
        let mut phone = (state.build)();
        let button = bar_buttons(&phone.frame())[at].clone();
        // *Move…* and *Make a rule…* open the palette too, at their step: the entry that opens
        // it at the top is the `search` one.
        if button.icon.as_deref() == Some("search") {
            searches += 1;
            if phone.tap(button.action) == [Command::OpenPalette {}] {
                entries += 1;
            }
        }
    }
    (entries, searches)
}

#[test]
fn budget_b_a_palette_entry_in_every_state() {
    for state in states() {
        let (entries, searches) = palette_entries(&state);
        match state.exempt {
            None => {
                assert_eq!(
                    entries,
                    1,
                    "{}: {entries} palette entries in the bars {}",
                    state.name,
                    describe(&bar_buttons(&(state.build)().frame()))
                );
                assert_eq!(
                    searches, entries,
                    "{}: a search icon that does not open the palette",
                    state.name
                );
            }
            Some(reason) => assert_eq!(
                (entries, searches),
                (0, 0),
                "{} is exempt ({reason}), yet a bar has a palette entry: drop the exemption",
                state.name
            ),
        }
    }
}

#[test]
fn every_state_with_a_bar_is_either_checked_or_listed_as_exempt() {
    // The exemptions are few and each says why.
    let exempt: Vec<_> = states()
        .into_iter()
        .filter(|s| s.exempt.is_some())
        .map(|s| s.name)
        .collect();
    assert_eq!(exempt, ["About", "no accounts"]);
}

/// The settings root of a phone with or without a hardware keyboard.
fn settings_root(keyboard: bool) -> Phone {
    root(keyboard)
}

#[test]
fn budget_c_every_settings_section_is_one_tap_from_the_root() {
    for keyboard in [true, false] {
        let who = if keyboard { "keyboard" } else { "no keyboard" };
        let mut phone = settings_root(keyboard);
        let frame = phone.frame();
        let page = frame.settings.unwrap();
        assert_eq!(page.section, "index", "{who}");
        assert!(page.groups.is_empty() && page.tabs.is_empty(), "{who}");
        let mut labels: Vec<_> = page.sections.iter().map(|e| e.label.as_str()).collect();
        labels.sort_unstable();
        let mut want = [
            "About",
            "Accounts",
            "Appearance",
            "General",
            "Keyboard",
            "Notifications & background",
            "Rules",
        ];
        want.sort_unstable();
        assert_eq!(labels, want, "{who}: the root lists the seven sections");
        assert_eq!(
            page.sections.last().map(|e| e.folded),
            Some(!keyboard),
            "{who}: Keyboard is folded, and last, without a keyboard"
        );

        for at in 0..page.sections.len() {
            let page = phone.frame().settings.unwrap();
            let target = &page.sections[at];
            let label = target.label.clone();
            // One tap.
            phone.tap(target.action);
            let frame = phone.frame();
            if label == "About" {
                assert!(frame.about.is_some(), "{who}: About is one tap away");
            } else {
                assert_eq!(frame.top_bar.title, label, "{who}: {label} is one tap away");
                let shown = frame.settings.unwrap();
                assert_ne!(shown.section, "index", "{who}: {label}");
                assert!(
                    shown.tabs.is_empty() && shown.sections.is_empty(),
                    "{who}: {label} shows its groups alone"
                );
                assert!(
                    !shown.groups.is_empty() && shown.groups.iter().all(|g| !g.title.is_empty()),
                    "{who}: {label} has groups under captions"
                );
            }
            // One Back.
            phone.event(UiEvent::Back);
            let frame = phone.frame();
            assert!(frame.about.is_none(), "{who}: {label}");
            let back = frame.settings.unwrap_or_else(|| {
                panic!("{who}: Back from {label} closed the settings instead of the section")
            });
            assert_eq!(back.section, "index", "{who}: Back from {label}");
            assert_eq!(back.sections.len(), 7, "{who}: Back from {label}");
        }

        // The wave's exit criterion, literally: Rules is one tap from the root.
        let mut phone = settings_root(keyboard);
        let rules = entry(&mut phone, "Rules");
        assert_eq!(
            phone.tap(rules),
            [Command::OpenSettings {
                section: Some(SettingsSection::Rules)
            }]
        );
        assert_eq!(phone.frame().settings.unwrap().section, "rules", "{who}");
    }
}
