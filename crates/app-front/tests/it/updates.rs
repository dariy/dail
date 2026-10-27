//! The update panel, badge and first-run question (Wave 7 F1–F3).

use app_core::{
    InstallKind, UpdateArtifact, UpdateChecks, UpdateHost, UpdateRelease, UpdateStatus,
};
use app_front::palette::Flow;
use app_front::updates::{
    Action, INTERVALS, answer, ask_first_run, badge, interval_label, panel, version,
};

use crate::support::synced_state;

fn host(install: InstallKind) -> UpdateHost {
    UpdateHost {
        version: "0.1.0".to_owned(),
        target: "x86_64-unknown-linux-gnu".to_owned(),
        install,
    }
}

fn release() -> UpdateRelease {
    UpdateRelease {
        version: "0.2.0".to_owned(),
        published_ms: 0,
        notes_url: "https://github.com/dariy/dail/releases/tag/v0.2.0".to_owned(),
        artifact: Some(UpdateArtifact {
            target: "x86_64-unknown-linux-gnu".to_owned(),
            kind: "appimage".to_owned(),
            url: "https://github.com/dariy/dail/releases/download/v0.2.0/a".to_owned(),
            sha256: "0".repeat(64),
            size: 1,
        }),
    }
}

#[test]
fn a_build_without_a_host_offers_nothing() {
    let state = synced_state();
    let panel = panel(&state, 0, "0.0.9");
    assert_eq!(panel.version, "dail 0.0.9");
    assert!(panel.actions.is_empty());
    assert_eq!(panel.checked, None);
    assert!(!ask_first_run(&state));
    assert_eq!(badge(&state), None);
}

#[test]
fn the_panel_follows_the_status() {
    let mut state = synced_state();
    state.update.host = Some(host(InstallKind::Appimage));
    assert_eq!(version(&state, "x"), "dail 0.1.0");
    let p = panel(&state, 0, "x");
    assert_eq!(p.status, "Not checked yet");
    assert_eq!(p.actions, vec![Action::Check]);

    state.update.last_check_ms = Some(0);
    let p = panel(&state, 3 * 3_600_000, "x");
    assert_eq!(p.status, "dail is up to date");
    assert_eq!(p.checked.as_deref(), Some("checked 3h ago"));

    state.update.status = UpdateStatus::Available { release: release() };
    let p = panel(&state, 0, "x");
    assert_eq!(p.status, "dail 0.2.0 is available");
    assert_eq!(
        p.actions,
        vec![
            Action::Download,
            Action::Open(release().notes_url),
            Action::Skip("0.2.0".to_owned())
        ]
    );
    assert_eq!(badge(&state).as_deref(), Some("Update 0.2.0"));

    state.update.status = UpdateStatus::Ready {
        release: release(),
        path: "/tmp/a".to_owned(),
    };
    let p = panel(&state, 0, "x");
    assert_eq!(p.actions.first(), Some(&Action::Install));
    // Installing asks first.
    assert!(matches!(Action::Install.flow(&state), Flow::Ask(_)));
    assert_eq!(Action::Install.command(), None);

    state.update.status = UpdateStatus::Installed {
        version: "0.2.0".to_owned(),
    };
    assert_eq!(badge(&state).as_deref(), Some("Restart to update"));
    assert!(panel(&state, 0, "x").actions.is_empty());

    state.update.status = UpdateStatus::Failed {
        reason: "bad signature".to_owned(),
    };
    let p = panel(&state, 0, "x");
    assert_eq!(p.status, "Update failed: bad signature");
    assert_eq!(p.actions, vec![Action::Check]);
}

#[test]
fn mobile_installs_only_open_the_release_page() {
    let mut state = synced_state();
    state.update.host = Some(host(InstallKind::Android));
    state.update.status = UpdateStatus::Available { release: release() };
    let p = panel(&state, 0, "x");
    assert!(!p.actions.contains(&Action::Download));
    assert!(matches!(
        Action::Open(String::new()).flow(&state),
        Flow::Hidden
    ));
}

#[test]
fn the_question_is_asked_once_after_setup() {
    let mut state = synced_state();
    state.update.host = Some(host(InstallKind::Binary));
    assert!(ask_first_run(&state));
    let mut empty = app_core::AppState::default();
    empty.update.host = state.update.host.clone();
    assert!(
        !ask_first_run(&empty),
        "not during the first account's setup"
    );
    for (on, checks) in [(true, UpdateChecks::On), (false, UpdateChecks::Off)] {
        assert_eq!(
            answer(on),
            app_core::Command::SetUpdatePolicy {
                checks: Some(checks),
                interval_hours: None,
            }
        );
        state.settings.updates.checks = checks;
        assert!(!ask_first_run(&state));
    }
}

#[test]
fn intervals_are_within_the_policy() {
    for (hours, label) in INTERVALS {
        assert!(
            (app_core::UpdatePolicy::MIN_INTERVAL_HOURS
                ..=app_core::UpdatePolicy::MAX_INTERVAL_HOURS)
                .contains(&hours)
        );
        assert_eq!(interval_label(hours), label);
    }
    assert_eq!(interval_label(48), "every 48 h");
}
