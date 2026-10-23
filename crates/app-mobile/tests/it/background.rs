//! Leaving the app (Wave 4 A5): the host is told the background schedule, and a network that
//! comes back while the app is visible syncs.

use app_core::{Command, SettingKey};
use app_mobile::{External, UiEvent};

use crate::support::{Phone, requests_of};

fn schedules(phone: &mut Phone) -> Vec<External> {
    phone
        .mobile
        .externals()
        .into_iter()
        .filter(|e| matches!(e, External::Schedule { .. }))
        .collect()
}

#[test]
fn the_schedule_comes_with_the_first_frame_and_each_change() {
    let mut phone = Phone::inbox(360, 800);
    let _ = phone.frame();
    assert_eq!(
        schedules(&mut phone),
        [External::Schedule {
            sync: 1,
            interval_mins: 30
        }]
    );
    let _ = phone.frame();
    assert!(schedules(&mut phone).is_empty(), "only when it changes");
    phone.dispatch(Command::SetSetting {
        key: SettingKey::BackgroundIntervalMins,
        value: 60.into(),
    });
    phone.dispatch(Command::SetSetting {
        key: SettingKey::BackgroundSync,
        value: 0.into(),
    });
    let _ = phone.frame();
    assert_eq!(
        schedules(&mut phone),
        [External::Schedule {
            sync: 0,
            interval_mins: 60
        }]
    );
}

#[test]
fn a_network_that_comes_back_syncs_only_while_visible() {
    let mut phone = Phone::inbox(360, 800);
    let _ = phone.frame();
    phone.event(UiEvent::Visible { visible: false });
    phone.event(UiEvent::Online);
    assert!(phone.take().is_empty(), "off screen: the jobs do it");
    phone.event(UiEvent::Visible { visible: true });
    phone.event(UiEvent::Online);
    assert_eq!(
        requests_of(&phone.take()),
        [Command::SyncNow { account: None }]
    );
}
