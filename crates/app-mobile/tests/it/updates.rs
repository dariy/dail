//! Wave 7 F2/F3: the first-run question about update checks and the banner of a new version,
//! which opens the release page (the apps never install).

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use app_core::{
    Command, Event, InstallKind, UpdateArtifact, UpdateChecks, UpdateHost, UpdateManifest,
};
use app_mobile::{External, Frame};

use crate::support::Phone;

/// The release page of the sample release.
pub const NOTES: &str = "https://github.com/dariy/dail/releases/tag/v9.0.0";

/// A phone whose host reported an Android build of 0.1.0.
pub fn android_phone() -> Phone {
    let mut phone = Phone::inbox(360, 800);
    phone.ingest(Event::UpdateHostDetected {
        host: UpdateHost {
            version: "0.1.0".to_owned(),
            target: "android".to_owned(),
            install: InstallKind::Android,
        },
    });
    phone
}

/// `phone` after a check that found 9.0.0.
pub fn with_release(phone: &mut Phone) {
    phone.dispatch(Command::CheckForUpdates {});
    phone.ingest(Event::UpdateChecked {
        manifest: UpdateManifest {
            version: "9.0.0".to_owned(),
            published_ms: 0,
            notes_url: NOTES.to_owned(),
            artifacts: vec![UpdateArtifact {
                target: "android".to_owned(),
                kind: "apk-arm64-v8a".to_owned(),
                url: "https://github.com/dariy/dail/releases/download/v9.0.0/a.apk".to_owned(),
                sha256: "0".repeat(64),
                size: 1,
            }],
        },
    });
}

fn banner(frame: &Frame) -> &app_mobile::frame::Banner {
    frame.banner.as_ref().unwrap()
}

#[test]
fn the_question_waits_for_a_build_that_can_check() {
    let mut phone = Phone::inbox(360, 800);
    assert!(phone.frame().banner.is_none());
    let mut phone = android_phone();
    let frame = phone.frame();
    let question = banner(&frame);
    assert!(question.info);
    assert_eq!(question.text, app_front::updates::QUESTION);
    assert!(question.dismiss.is_some());
}

#[test]
fn answering_sets_the_policy_and_the_question_goes() {
    for (button, want) in [(true, UpdateChecks::On), (false, UpdateChecks::Off)] {
        let mut phone = android_phone();
        let frame = phone.frame();
        let question = banner(&frame);
        let tap = if button {
            question.action.as_ref().unwrap().action
        } else {
            question.dismiss.as_ref().unwrap().action
        };
        phone.tap(tap);
        assert_eq!(phone.state().settings.updates.checks, want);
        assert!(phone.frame().banner.is_none());
    }
}

#[test]
fn a_new_version_opens_its_release_page() {
    let mut phone = android_phone();
    phone.dispatch(app_front::updates::answer(true));
    with_release(&mut phone);
    let frame = phone.frame();
    let news = banner(&frame);
    assert!(news.info);
    assert_eq!(news.text, "dail 9.0.0 is available");
    phone.tap(news.action.as_ref().unwrap().action);
    assert!(phone.externals().contains(&External::OpenUrl {
        url: NOTES.to_owned()
    }));
}

#[test]
fn skipping_a_version_hides_its_banner() {
    let mut phone = android_phone();
    phone.dispatch(app_front::updates::answer(false));
    with_release(&mut phone);
    let frame = phone.frame();
    phone.tap(banner(&frame).dismiss.as_ref().unwrap().action);
    assert_eq!(
        phone.state().settings.updates.skipped_version.as_deref(),
        Some("9.0.0")
    );
    assert!(phone.frame().banner.is_none());
}
