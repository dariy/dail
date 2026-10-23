//! Received attachments and pictures: previews in the app (image, text, none), *Save* and *Save
//! all* through the host's pickers, no "open with", and inline pictures only for senders in
//! `images_from`.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::Command;
use app_mobile::event::SaveFile;
use app_mobile::frame::{DocBlock, PreviewKind, ReaderMessage};
use app_mobile::{External, preview_text};

use crate::support::{Phone, with_files};

fn first(phone: &mut Phone) -> ReaderMessage {
    phone.frame().reader.unwrap().messages[0].clone()
}

#[test]
fn attachments_preview_by_kind_and_save_through_the_host() {
    let mut phone = with_files();
    let message = first(&mut phone);
    let names: Vec<&str> = message
        .attachments
        .iter()
        .map(|a| a.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["chart.png", "notes.txt", "q3.pdf"],
        "inline parts are not listed"
    );
    for (i, kind) in [PreviewKind::Image, PreviewKind::Text, PreviewKind::None]
        .into_iter()
        .enumerate()
    {
        let message = first(&mut phone);
        assert!(phone.tap(message.attachments[i].open).is_empty());
        let frame = phone.frame();
        let preview = frame.preview.as_ref().unwrap();
        assert_eq!(preview.kind, kind);
        assert_eq!(
            (preview.blob.as_str(), preview.part),
            ("b0", message.attachments[i].part)
        );
        assert_eq!(preview.note.is_some(), kind == PreviewKind::None);
        assert!(frame.back);
        phone.take();
        assert!(phone.event(app_mobile::UiEvent::Back).is_empty());
        assert!(phone.frame().preview.is_none());
    }
    let message = first(&mut phone);
    phone.tap(message.attachments[2].save);
    assert_eq!(
        phone.externals(),
        [External::SaveFiles {
            blob: "b0".to_owned(),
            files: vec![SaveFile {
                part: 4,
                name: "q3.pdf".to_owned(),
                mime: "application/pdf".to_owned()
            }]
        }]
    );
    let all = message.save_all.unwrap();
    assert_eq!(all.label, "Save all (3)");
    phone.tap(all.action);
    let externals = phone.externals();
    assert!(
        matches!(&externals[..], [External::SaveFiles { files, .. }] if files.len() == 3),
        "{externals:?}"
    );
}

#[test]
fn pictures_show_only_for_senders_the_user_allowed() {
    let mut phone = with_files();
    let message = first(&mut phone);
    let images = message.images.clone().unwrap();
    assert!(!images.shown);
    let has_image = |message: &ReaderMessage| {
        message.body.as_ref().unwrap().iter().any(|b| {
            matches!(b, DocBlock::Paragraph { spans } if spans.iter().any(|s| s.image.is_some()))
        })
    };
    assert!(!has_image(&message), "alt text until allowed");
    let got = phone.tap(images.toggle.action);
    assert!(
        matches!(&got[..], [Command::ShowImagesFrom { .. }]),
        "{got:?}"
    );
    let message = first(&mut phone);
    assert!(message.images.as_ref().unwrap().shown);
    assert!(has_image(&message));
    let image = message
        .body
        .unwrap()
        .into_iter()
        .find_map(|b| match b {
            DocBlock::Paragraph { spans } => spans.into_iter().find_map(|s| s.image),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        (image.blob.as_str(), image.cid.as_str()),
        ("b0", "chart@example.org")
    );
}

#[test]
fn text_previews_are_cleaned_and_capped() {
    let text = preview_text(b"ok\x1b]52;c;x\x07\r\nline \xe2\x80\xae2\xff");
    assert!(
        !text.contains(['\u{1b}', '\u{7}', '\u{202e}', '\r']),
        "{text:?}"
    );
    assert!(text.starts_with("ok") && text.contains("\nline "));
    let big = preview_text(&vec![b'a'; 300 * 1024]);
    assert!(big.ends_with('…') && big.len() < 300 * 1024);
}
