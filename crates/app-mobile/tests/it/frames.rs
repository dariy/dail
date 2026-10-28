//! Frames over the `basic` fixture as JSON snapshots, at a phone's, a small tablet's and a large
//! tablet's size.

use app_core::{Command, CommandName, Pane};
use app_mobile::frame::PaneKind;
use app_mobile::{Class, UiEvent};

use crate::support::{Phone, SIZES, inbox_of, synced_state};

fn name((w, h): (u32, u32), what: &str) -> String {
    format!("{what}_{w}x{h}")
}

#[test]
fn inbox_at_every_size() {
    for size in SIZES {
        let mut phone = Phone::inbox(size.0, size.1);
        insta::assert_json_snapshot!(name(size, "inbox"), phone.frame());
    }
}

#[test]
fn open_thread_at_every_size() {
    for size in SIZES {
        let mut phone = Phone::inbox(size.0, size.1);
        let (_, threads) = inbox_of(phone.state());
        phone.dispatch(Command::OpenThread { id: threads[0] });
        insta::assert_json_snapshot!(name(size, "thread"), phone.frame());
    }
}

#[test]
fn drawer_at_every_size() {
    for size in SIZES {
        let mut phone = Phone::inbox(size.0, size.1);
        phone.dispatch(Command::FocusPane {
            pane: Pane::Sidebar,
        });
        insta::assert_json_snapshot!(name(size, "drawer"), phone.frame());
    }
}

#[test]
fn no_accounts() {
    let mut phone = Phone::new(app_core::AppState::default(), 360, 800);
    insta::assert_json_snapshot!("no_accounts", phone.frame());
}

#[test]
fn window_classes_choose_the_panes() {
    let expect = [
        (360, Class::Compact, vec![PaneKind::List]),
        (700, Class::Medium, vec![PaneKind::List, PaneKind::Reader]),
        (
            1280,
            Class::Expanded,
            vec![PaneKind::Drawer, PaneKind::List, PaneKind::Reader],
        ),
    ];
    for (width, class, panes) in expect {
        let mut phone = Phone::inbox(width, 800);
        let frame = phone.frame();
        assert_eq!(frame.layout.class, class);
        assert_eq!(frame.layout.panes, panes);
    }
    // A collapsed sidebar is a rail on a wide window.
    let mut phone = Phone::inbox(1280, 800);
    phone.dispatch(Command::ToggleSidebar {});
    let frame = phone.frame();
    assert_eq!(
        frame.layout.panes,
        [PaneKind::Drawer, PaneKind::List, PaneKind::Reader]
    );
    assert!(frame.drawer.unwrap().rail);
}

#[test]
fn the_list_is_a_window_around_the_visible_rows() {
    let mut phone = Phone::inbox(360, 800);
    let frame = phone.frame();
    let list = frame.list.unwrap();
    assert_eq!(list.offset, 0);
    assert_eq!(list.rows.len(), list.total.min(50));
    let total = inbox_of(&synced_state()).1.len();
    assert_eq!(list.total, total);
}

#[test]
fn a_pinch_gives_the_message_more_room_one_step_at_a_time() {
    // MISC-6: 1280×800 shows the sidebar (a rail in landscape), the list and the message.
    let mut phone = Phone::inbox(1280, 800);
    let (_, threads) = crate::support::inbox_of(phone.state());
    phone.dispatch(app_core::Command::OpenThread { id: threads[0] });
    let panes = |phone: &mut Phone| phone.frame().layout.panes;
    assert_eq!(
        panes(&mut phone),
        [PaneKind::Drawer, PaneKind::List, PaneKind::Reader]
    );
    phone.event(UiEvent::Pinch { zoom_in: true });
    assert_eq!(panes(&mut phone), [PaneKind::List, PaneKind::Reader]);
    phone.event(UiEvent::Pinch { zoom_in: true });
    assert_eq!(panes(&mut phone), [PaneKind::Reader]);
    phone.event(UiEvent::Pinch { zoom_in: true });
    assert_eq!(panes(&mut phone), [PaneKind::Reader], "no further");
    // The step stays with the next thread.
    phone.dispatch(app_core::Command::OpenThread { id: threads[1] });
    assert_eq!(panes(&mut phone), [PaneKind::Reader]);
    phone.event(UiEvent::Pinch { zoom_in: false });
    assert_eq!(panes(&mut phone), [PaneKind::List, PaneKind::Reader]);
    // Without a message open, the list keeps its panes.
    phone.dispatch(app_core::Command::CloseThread {});
    assert!(panes(&mut phone).contains(&PaneKind::List));
}

#[test]
fn a_landscape_window_switches_the_sidebar_between_tree_and_rail() {
    // MISC-6 keeps the left pane in landscape; MISC-11 lets the user pick its style.
    let mut phone = Phone::inbox(800, 360);
    let frame = phone.frame();
    assert_eq!(frame.layout.panes[0], PaneKind::Drawer);
    let drawer = frame.drawer.unwrap();
    assert!(!drawer.rail, "the full tree by default");
    let style = drawer.style.unwrap();
    assert_eq!(style.label, "Icons only");
    assert_eq!(phone.tap(style.action), [Command::ToggleSidebar {}]);
    assert!(phone.state().view.sidebar_collapsed, "persisted in [view]");
    let frame = phone.frame();
    assert_eq!(frame.layout.panes[0], PaneKind::Drawer);
    let drawer = frame.drawer.unwrap();
    assert!(drawer.rail, "icons only");
    let style = drawer.style.unwrap();
    assert_eq!(style.label, "Show folder names");
    phone.tap(style.action);
    assert!(!phone.frame().drawer.unwrap().rail);

    let mut phone = Phone::inbox(700, 1000);
    let frame = phone.frame();
    assert!(
        frame.drawer.is_none_or(|d| !d.rail && d.style.is_none()),
        "portrait: no rail, no switch"
    );
}

#[test]
fn the_message_pane_maximizes_and_restores() {
    // MISC-14: a button, not only the pinch.
    let mut phone = Phone::inbox(1280, 800);
    let (_, threads) = inbox_of(phone.state());
    phone.dispatch(Command::OpenThread { id: threads[0] });
    let frame = phone.frame();
    assert!(frame.layout.panes.len() > 1);
    let maximize = frame.reader.unwrap().maximize.unwrap();
    assert_eq!(maximize.label, "Maximize message");
    assert!(
        phone.tap(maximize.action).is_empty(),
        "no command: the presenter's"
    );
    let frame = phone.frame();
    assert_eq!(frame.layout.panes, [PaneKind::Reader]);
    assert!(frame.top_bar.actions.is_empty(), "a bare top bar");
    let restore = frame.reader.unwrap().maximize.unwrap();
    assert_eq!(restore.label, "Restore panes");
    // It stays with the next thread.
    phone.dispatch(Command::OpenThread { id: threads[1] });
    assert_eq!(phone.frame().layout.panes, [PaneKind::Reader]);
    let restore = phone.frame().reader.unwrap().maximize.unwrap();
    phone.tap(restore.action);
    assert!(phone.frame().layout.panes.len() > 1);

    // A phone shows the message alone already: nothing to maximize.
    let mut phone = Phone::inbox(360, 800);
    phone.dispatch(Command::OpenThread { id: threads[0] });
    assert!(phone.frame().reader.unwrap().maximize.is_none());
}

#[test]
fn a_phone_shows_the_message_as_a_sheet_over_the_list() {
    // MISC-17: the list stays (and keeps its place); the message rises over it.
    let mut phone = Phone::inbox(360, 800);
    let list_title = phone.frame().top_bar.title;
    let (_, threads) = inbox_of(phone.state());
    phone.dispatch(Command::OpenThread { id: threads[0] });
    let frame = phone.frame();
    assert_eq!(frame.layout.panes, [PaneKind::List]);
    assert_eq!(frame.layout.sheet, Some(PaneKind::Reader));
    assert!(frame.list.is_some() && frame.reader.is_some());
    assert_eq!(frame.top_bar.title, list_title, "the list's top bar");
    assert!(frame.fab.is_none(), "no floating button under the sheet");
    assert!(frame.back, "Back closes the sheet");
    // The sheet's close (a drag down) is the ×: `close_thread`.
    let close = frame.reader.unwrap().close;
    assert_eq!(
        phone.tap(close.action).first().map(Command::command_name),
        Some(CommandName::CloseThread)
    );

    // Medium and expanded windows keep the message as a pane.
    for width in [700, 1280] {
        let mut phone = Phone::inbox(width, 800);
        phone.dispatch(Command::OpenThread { id: threads[0] });
        let frame = phone.frame();
        assert!(frame.layout.sheet.is_none());
        assert!(frame.layout.panes.contains(&PaneKind::Reader));
    }
}
