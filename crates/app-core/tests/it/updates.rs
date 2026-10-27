//! Updates of dail itself (Wave 7 C1–C3): the five commands, their events, the version
//! comparison, the agent policy and the periodic check.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::update::{is_newer, parse_version};
use app_core::{
    Command, CoreError, Ctx, Effect, Engine, Event, InstallKind, Origin, UpdateArtifact,
    UpdateChecks, UpdateHost, UpdateManifest, UpdateStatus,
};

const T0: i64 = 1_700_000_000_000;
const HOUR: i64 = 3_600_000;

fn at(ms: i64, origin: Origin) -> Ctx {
    Ctx {
        now_ms: ms,
        id_seed: u128::try_from(ms).unwrap() << 64,
        origin,
    }
}

fn host(install: InstallKind) -> UpdateHost {
    let target = match install {
        InstallKind::Msi => "x86_64-pc-windows-msvc",
        InstallKind::Android => "android",
        InstallKind::Ios => "ios",
        _ => "x86_64-unknown-linux-gnu",
    };
    UpdateHost {
        version: "0.1.0".to_owned(),
        target: target.to_owned(),
        install,
    }
}

fn artifact(target: &str, kind: &str) -> UpdateArtifact {
    UpdateArtifact {
        target: target.to_owned(),
        kind: kind.to_owned(),
        url: format!("https://github.com/dariy/dail/releases/download/v0.2.0/{kind}"),
        sha256: "00".repeat(32),
        size: 3,
    }
}

fn manifest(version: &str) -> UpdateManifest {
    UpdateManifest {
        version: version.to_owned(),
        published_ms: T0 - HOUR,
        notes_url: format!("https://github.com/dariy/dail/releases/tag/v{version}"),
        artifacts: vec![
            artifact("x86_64-unknown-linux-gnu", "appimage"),
            artifact("x86_64-unknown-linux-gnu", "deb"),
            artifact("x86_64-unknown-linux-gnu", "exe"),
            artifact("x86_64-pc-windows-msvc", "msi"),
            artifact("android", "apk-arm64-v8a"),
            artifact("ios", "ipa"),
        ],
    }
}

fn engine(install: InstallKind) -> Engine {
    let mut engine = Engine::default();
    let outcome = engine
        .ingest(
            Event::UpdateHostDetected {
                host: host(install),
            },
            &at(T0, Origin::Runtime),
        )
        .unwrap();
    // Checks are not on yet: nothing starts.
    assert!(outcome.effects.is_empty());
    engine
}

fn run(engine: &mut Engine, cmd: Command, ms: i64) -> Result<Vec<Effect>, CoreError> {
    engine
        .dispatch(cmd, &at(ms, Origin::Gui))
        .map(|outcome| outcome.effects)
}

fn feed(engine: &mut Engine, event: Event, ms: i64) -> Vec<Effect> {
    engine
        .ingest(event, &at(ms, Origin::Runtime))
        .unwrap()
        .effects
}

/// A check that finds `version`.
fn found(engine: &mut Engine, version: &str, ms: i64) {
    let effects = run(engine, Command::CheckForUpdates {}, ms).unwrap();
    assert_eq!(
        effects,
        [Effect::CheckForUpdates {
            running: "0.1.0".to_owned()
        }]
    );
    assert_eq!(engine.state().update.status, UpdateStatus::Checking);
    feed(
        engine,
        Event::UpdateChecked {
            manifest: manifest(version),
        },
        ms + 1000,
    );
}

fn invalid(result: Result<Vec<Effect>, CoreError>, field: &str) {
    match result {
        Err(CoreError::InvalidArgument { field: f, .. }) => assert_eq!(f, field),
        other => panic!("expected invalid_argument {field}, got {other:?}"),
    }
}

#[test]
fn versions_compare_as_numbers() {
    assert_eq!(parse_version("0.10.2"), Some((0, 10, 2)));
    assert_eq!(parse_version("1.2"), None);
    assert_eq!(parse_version("1.2.3.4"), None);
    assert_eq!(parse_version("1.2.x"), None);
    assert!(is_newer("0.10.0", "0.9.9"));
    assert!(is_newer("1.0.0", "0.99.99"));
    assert!(!is_newer("0.1.0", "0.1.0"));
    assert!(!is_newer("0.0.9", "0.1.0"));
    assert!(!is_newer("garbage", "0.1.0"));
}

#[test]
fn check_download_install_on_an_appimage() {
    let mut e = engine(InstallKind::Appimage);
    found(&mut e, "0.2.0", T0);
    let UpdateStatus::Available { release } = e.state().update.status.clone() else {
        panic!("{:?}", e.state().update.status);
    };
    assert_eq!(release.version, "0.2.0");
    assert_eq!(release.artifact.as_ref().unwrap().kind, "appimage");
    assert_eq!(e.state().update.last_check_ms, Some(T0 + 1000));

    let effects = run(&mut e, Command::DownloadUpdate {}, T0 + 2000).unwrap();
    assert_eq!(
        effects,
        [Effect::DownloadUpdate {
            version: "0.2.0".to_owned(),
            artifact: release.artifact.clone().unwrap(),
        }]
    );
    assert!(matches!(
        e.state().update.status,
        UpdateStatus::Downloading { .. }
    ));
    feed(
        &mut e,
        Event::UpdateDownloaded {
            version: "0.2.0".to_owned(),
            path: "/p/updates/dail-0.2.0-x86_64.AppImage".to_owned(),
        },
        T0 + 3000,
    );
    assert!(matches!(
        e.state().update.status,
        UpdateStatus::Ready { .. }
    ));

    let effects = run(&mut e, Command::InstallUpdate { confirm: true }, T0 + 4000).unwrap();
    assert_eq!(
        effects,
        [Effect::InstallUpdate {
            version: "0.2.0".to_owned(),
            install: InstallKind::Appimage,
            path: "/p/updates/dail-0.2.0-x86_64.AppImage".to_owned(),
        }]
    );
    feed(
        &mut e,
        Event::UpdateInstalled {
            version: "0.2.0".to_owned(),
        },
        T0 + 5000,
    );
    assert_eq!(
        e.state().update.status,
        UpdateStatus::Installed {
            version: "0.2.0".to_owned()
        }
    );
}

#[test]
fn each_install_kind_takes_its_artifact() {
    for (install, kind) in [
        (InstallKind::Binary, "exe"),
        (InstallKind::Deb, "deb"),
        (InstallKind::Msi, "msi"),
        (InstallKind::Android, "apk-arm64-v8a"),
        (InstallKind::Ios, "ipa"),
    ] {
        let mut e = engine(install);
        found(&mut e, "0.2.0", T0);
        let UpdateStatus::Available { release } = &e.state().update.status else {
            panic!("{install:?}");
        };
        assert_eq!(release.artifact.as_ref().unwrap().kind, kind, "{install:?}");
    }
}

#[test]
fn mobile_installs_only_notify() {
    let mut e = engine(InstallKind::Android);
    found(&mut e, "0.2.0", T0);
    invalid(run(&mut e, Command::DownloadUpdate {}, T0 + 2000), "update");
    assert!(matches!(
        e.state().update.status,
        UpdateStatus::Available { .. }
    ));
}

#[test]
fn nothing_newer_is_idle() {
    let mut e = engine(InstallKind::Binary);
    found(&mut e, "0.1.0", T0);
    assert_eq!(e.state().update.status, UpdateStatus::Idle);
    found(&mut e, "0.0.9", T0 + 5000);
    assert_eq!(e.state().update.status, UpdateStatus::Idle);
}

#[test]
fn failures_surface_in_state() {
    let mut e = engine(InstallKind::Binary);
    run(&mut e, Command::CheckForUpdates {}, T0).unwrap();
    feed(
        &mut e,
        Event::UpdateFailed {
            reason: "the manifest signature does not verify".to_owned(),
        },
        T0 + 1000,
    );
    assert_eq!(
        e.state().update.status,
        UpdateStatus::Failed {
            reason: "the manifest signature does not verify".to_owned()
        }
    );
    assert_eq!(e.state().update.last_check_ms, Some(T0 + 1000));
    // The effects report their failures as `update_failed`.
    let effect = Effect::DownloadUpdate {
        version: "0.2.0".to_owned(),
        artifact: artifact("x", "exe"),
    };
    assert_eq!(
        effect.failure_event("sha256 mismatch".to_owned()),
        Event::UpdateFailed {
            reason: "sha256 mismatch".to_owned()
        }
    );
}

#[test]
fn commands_refuse_the_wrong_state() {
    // No host: the runtime cannot update this build.
    let mut bare = Engine::default();
    invalid(run(&mut bare, Command::CheckForUpdates {}, T0), "update");

    let mut e = engine(InstallKind::Binary);
    invalid(run(&mut e, Command::DownloadUpdate {}, T0), "update");
    invalid(
        run(&mut e, Command::InstallUpdate { confirm: true }, T0),
        "update",
    );
    assert!(matches!(
        run(&mut e, Command::InstallUpdate { confirm: false }, T0),
        Err(CoreError::ConfirmationRequired { .. })
    ));
    invalid(
        run(
            &mut e,
            Command::SkipUpdateVersion {
                version: Some("next".to_owned()),
            },
            T0,
        ),
        "version",
    );
    invalid(
        run(
            &mut e,
            Command::SetUpdatePolicy {
                checks: None,
                interval_hours: Some(0),
            },
            T0,
        ),
        "interval_hours",
    );
    invalid(
        run(
            &mut e,
            Command::SetUpdatePolicy {
                checks: None,
                interval_hours: Some(721),
            },
            T0,
        ),
        "interval_hours",
    );
    // A second check while one runs starts nothing.
    run(&mut e, Command::CheckForUpdates {}, T0).unwrap();
    assert!(
        run(&mut e, Command::CheckForUpdates {}, T0)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn agents_cannot_install() {
    let mut e = engine(InstallKind::Binary);
    found(&mut e, "0.2.0", T0);
    // Agents may check and download.
    e.dispatch(Command::DownloadUpdate {}, &at(T0 + 2000, Origin::Mcp))
        .unwrap();
    feed(
        &mut e,
        Event::UpdateDownloaded {
            version: "0.2.0".to_owned(),
            path: "/p/updates/dail".to_owned(),
        },
        T0 + 3000,
    );
    let refused = e.dispatch(
        Command::InstallUpdate { confirm: true },
        &at(T0 + 4000, Origin::Mcp),
    );
    assert!(
        matches!(refused, Err(CoreError::OriginNotAllowed { .. })),
        "{refused:?}"
    );
    assert!(matches!(
        e.state().update.status,
        UpdateStatus::Ready { .. }
    ));
}

#[test]
fn skipping_hides_that_version_and_undoes() {
    let mut e = engine(InstallKind::Binary);
    found(&mut e, "0.2.0", T0);
    run(
        &mut e,
        Command::SkipUpdateVersion {
            version: Some("0.2.0".to_owned()),
        },
        T0 + 2000,
    )
    .unwrap();
    assert_eq!(e.state().update.status, UpdateStatus::Idle);
    assert_eq!(
        e.state().settings.updates.skipped_version.as_deref(),
        Some("0.2.0")
    );
    found(&mut e, "0.2.0", T0 + 3000);
    assert_eq!(e.state().update.status, UpdateStatus::Idle);
    // A newer one is offered.
    found(&mut e, "0.2.1", T0 + 5000);
    assert!(matches!(
        e.state().update.status,
        UpdateStatus::Available { .. }
    ));

    run(&mut e, Command::Undo {}, T0 + 7000).unwrap();
    assert_eq!(e.state().settings.updates.skipped_version, None);
    run(&mut e, Command::Redo {}, T0 + 8000).unwrap();
    assert_eq!(
        e.state().settings.updates.skipped_version.as_deref(),
        Some("0.2.0")
    );
}

#[test]
fn the_policy_round_trips_through_undo() {
    let mut e = engine(InstallKind::Binary);
    let effects = run(
        &mut e,
        Command::SetUpdatePolicy {
            checks: Some(UpdateChecks::Off),
            interval_hours: Some(48),
        },
        T0,
    )
    .unwrap();
    assert!(matches!(effects.as_slice(), [Effect::PersistState { .. }]));
    let policy = &e.state().settings.updates;
    assert_eq!(
        (policy.checks, policy.interval_hours),
        (UpdateChecks::Off, 48)
    );
    run(&mut e, Command::Undo {}, T0 + 1000).unwrap();
    let policy = &e.state().settings.updates;
    assert_eq!(
        (policy.checks, policy.interval_hours),
        (UpdateChecks::Unset, 24)
    );
    run(&mut e, Command::Redo {}, T0 + 2000).unwrap();
    assert_eq!(e.state().settings.updates.interval_hours, 48);
}

#[test]
fn periodic_checks_are_due_after_the_interval() {
    let mut e = engine(InstallKind::Binary);
    // Turning checks on with no check yet starts one at once.
    let effects = run(
        &mut e,
        Command::SetUpdatePolicy {
            checks: Some(UpdateChecks::On),
            interval_hours: Some(2),
        },
        T0,
    )
    .unwrap();
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::CheckForUpdates { .. })),
        "{effects:?}"
    );
    // The answer schedules the next check, two hours later.
    let effects = feed(
        &mut e,
        Event::UpdateChecked {
            manifest: manifest("0.1.0"),
        },
        T0 + 1000,
    );
    assert_eq!(
        effects,
        [Effect::ScheduleTick {
            at_ms: T0 + 1000 + 2 * HOUR
        }]
    );
    // Mail ticks before then start nothing and schedule no second tick for the same time.
    let effects = feed(&mut e, Event::Tick { now_ms: T0 + HOUR }, T0 + HOUR);
    assert!(
        !effects.iter().any(|e| matches!(
            e,
            Effect::CheckForUpdates { .. } | Effect::ScheduleTick { .. }
        )),
        "{effects:?}"
    );
    // The tick at the due time checks.
    let due = T0 + 1000 + 2 * HOUR;
    let effects = feed(&mut e, Event::Tick { now_ms: due }, due);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::CheckForUpdates { .. })),
        "{effects:?}"
    );
}

#[test]
fn a_restart_checks_when_due() {
    let mut e = Engine::default();
    run(
        &mut e,
        Command::SetUpdatePolicy {
            checks: Some(UpdateChecks::On),
            interval_hours: None,
        },
        T0,
    )
    .unwrap();
    // No host yet: nothing to check with.
    let effects = feed(
        &mut e,
        Event::UpdateHostDetected {
            host: host(InstallKind::Deb),
        },
        T0 + 1000,
    );
    assert_eq!(
        effects,
        [Effect::CheckForUpdates {
            running: "0.1.0".to_owned()
        }]
    );
}

#[test]
fn checks_stay_off_until_turned_on() {
    let mut e = engine(InstallKind::Binary);
    for checks in [UpdateChecks::Unset, UpdateChecks::Off] {
        if checks == UpdateChecks::Off {
            run(
                &mut e,
                Command::SetUpdatePolicy {
                    checks: Some(checks),
                    interval_hours: None,
                },
                T0,
            )
            .unwrap();
        }
        let later = T0 + 100 * HOUR;
        let effects = feed(&mut e, Event::Tick { now_ms: later }, later);
        assert!(
            !effects
                .iter()
                .any(|e| matches!(e, Effect::CheckForUpdates { .. })),
            "{checks:?}: {effects:?}"
        );
    }
}
