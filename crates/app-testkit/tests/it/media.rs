//! The `media` fixture: a camera photo and PDFs for the attachment previews (MISC-19).

// reason: test helpers build fixtures; a failure is the test failing.
#![allow(clippy::unwrap_used)]

use app_testkit::Fixture;

#[test]
fn the_media_fixture_carries_a_photo_and_pdfs() {
    assert!(Fixture::builtin_names().contains(&"media"));
    let fixture = Fixture::builtin("media").unwrap();
    let inbox = fixture.accounts[0]
        .mailboxes
        .iter()
        .find(|m| m.path == "INBOX")
        .unwrap();
    let raw = &inbox.messages.last().unwrap().raw;
    assert!(raw.contains("bob@example.org"));
    for name in ["photo.jpg", "report.pdf", "broken.pdf"] {
        assert!(raw.contains(name), "{name} is attached");
    }
    let photo = app_io::images::decode(app_testkit::media::PHOTO).unwrap();
    assert_eq!(
        (photo.width, photo.height),
        (2000, 3000),
        "downscaled, upright"
    );
}

/// Writes `fixtures/media/`: `cargo test -p app-testkit --test it media::regenerate -- --ignored`.
#[test]
#[ignore = "regenerates the committed media files"]
fn regenerate() {
    use image::{RgbImage, codecs::jpeg::JpegEncoder};
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/media");
    std::fs::create_dir_all(&dir).unwrap();
    // 6000×4000, landscape as stored, EXIF orientation 6: a portrait photo once upright.
    let image = RgbImage::from_fn(6000, 4000, |x, y| {
        image::Rgb([
            (x / 200 * 8) as u8,
            (y / 200 * 12) as u8,
            160 - (x / 300 * 6) as u8,
        ])
    });
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, 40)
        .encode_image(&image)
        .unwrap();
    let mut exif =
        b"Exif\0\0MM\0\x2a\0\0\0\x08\0\x01\x01\x12\0\x03\0\0\0\x01\0\x06\0\0\0\0\0\0".to_vec();
    let mut photo = jpeg[..2].to_vec();
    photo.extend_from_slice(&[0xFF, 0xE1]);
    photo.extend_from_slice(&u16::try_from(exif.len() + 2).unwrap().to_be_bytes());
    photo.append(&mut exif);
    photo.extend_from_slice(&jpeg[2..]);
    assert!(photo.len() < 800_000, "photo.jpg is {} bytes", photo.len());
    std::fs::write(dir.join("photo.jpg"), photo).unwrap();

    let mut text = String::from("BT /F1 18 Tf 72 770 Td (Quarterly report) Tj ET ");
    for line in 0..40 {
        text.push_str(&format!(
            "BT /F1 11 Tf 72 {} Td (Line {line}: budget, schedule and review for the quarter.) Tj ET ",
            740 - line * 16
        ));
    }
    let report = pdf(&[
        ("/MediaBox [0 0 595 842]", text),
        (
            "/MediaBox [0 0 842 595]",
            "0.2 0.4 0.8 rg 72 72 698 451 re f BT /F1 36 Tf 1 1 1 rg 120 280 Td (A landscape page) Tj ET".into(),
        ),
        (
            "/MediaBox [0 0 595 842]",
            "BT /F1 4 Tf 72 770 Td (Four-point text that must stay sharp when zoomed in.) Tj ET".into(),
        ),
    ]);
    std::fs::write(dir.join("report.pdf"), &report).unwrap();
    // A PDF header and its first object's start, then garbage: no page can be recovered.
    let mut broken = report[..40].to_vec();
    broken.extend_from_slice(&b"\x00\xff garbage, not a PDF any more\n".repeat(20));
    std::fs::write(dir.join("broken.pdf"), broken).unwrap();
}

fn pdf(pages: &[(&str, String)]) -> Vec<u8> {
    let n = pages.len();
    let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", 3 + 2 * i)).collect();
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        format!("<< /Type /Pages /Kids [{}] /Count {n} >>", kids.join(" ")),
    ];
    for (i, (entries, content)) in pages.iter().enumerate() {
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R {entries} /Contents {} 0 R /Resources << /Font << /F1 {} 0 R >> >> >>",
            4 + 2 * i,
            3 + 2 * n
        ));
        objects.push(format!(
            "<< /Length {} >>\nstream\n{content}\nendstream",
            content.len()
        ));
    }
    objects.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned());
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}
