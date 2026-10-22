//! Frames over the `basic` fixture as JSON snapshots, at a phone's, a small tablet's and a large
//! tablet's size.

use app_core::{Command, Pane};
use app_mobile::Class;
use app_mobile::frame::PaneKind;

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
