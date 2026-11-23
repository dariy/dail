//! `app_front::qr`: the matrix of a pairing URI decodes back to it, from pixels and from the
//! half-block text the terminal shows (SRV-QR1).

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_front::qr::{Ink, QUIET_ZONE, QrError, QrMatrix, dark_modules, half_blocks};

const URI: &str = "dail://pair?v=1&h=home.tail.ts.net&p=7443&c=ABCD2345\
&fp=5d41402abc4b2a76b9719d911017c592ae2f5e3c8a1b7d0f4e6c2a9b8d7f1e03&n=Phone%20%C3%A9";

/// Decodes a grey-scale image (`dark(x, y)` per pixel) with rqrr: the one code's text.
fn decode(side: usize, dark: impl Fn(usize, usize) -> bool) -> String {
    let mut image =
        rqrr::PreparedImage::prepare_from_greyscale(
            side,
            side,
            |x, y| {
                if dark(x, y) { 0 } else { 255 }
            },
        );
    let grids = image.detect_grids();
    assert_eq!(grids.len(), 1, "one QR code");
    grids[0].decode().unwrap().1
}

#[test]
fn the_matrix_decodes_back_to_the_text() {
    let matrix = QrMatrix::encode(URI).unwrap();
    assert_eq!(matrix.width() % 4, 1, "a QR side is 17 + 4 × version");
    let (side, dark) = dark_modules(&matrix, QUIET_ZONE);
    assert_eq!(side, matrix.width() + 2 * QUIET_ZONE);
    let px = 4;
    let set: std::collections::HashSet<(usize, usize)> = dark.into_iter().collect();
    let text = decode(side * px, |x, y| set.contains(&(x / px, y / px)));
    assert_eq!(text, URI);
}

/// Expands half-block lines back into pixels: a character is one module wide, two tall.
fn expand(lines: &[String], ink: Ink, px: usize) -> (usize, Vec<Vec<bool>>) {
    let width = lines[0].chars().count();
    assert!(lines.iter().all(|l| l.chars().count() == width));
    let mut modules = Vec::new();
    for line in lines {
        let (mut top, mut bottom) = (Vec::new(), Vec::new());
        for c in line.chars() {
            let (t, b) = match c {
                '█' => (true, true),
                '▀' => (true, false),
                '▄' => (false, true),
                ' ' => (false, false),
                other => panic!("unexpected {other:?}"),
            };
            // Inked means dark for Ink::Dark, light for Ink::Light.
            top.push(t == (ink == Ink::Dark));
            bottom.push(b == (ink == Ink::Dark));
        }
        modules.push(top);
        modules.push(bottom);
    }
    let side = width * px;
    let rows = modules.len();
    let pixels = (0..rows * px)
        .map(|y| (0..side).map(|x| modules[y / px][x / px]).collect())
        .collect();
    (side, pixels)
}

#[test]
fn the_half_blocks_scan_in_both_polarities() {
    let matrix = QrMatrix::encode(URI).unwrap();
    for ink in [Ink::Dark, Ink::Light] {
        let lines = half_blocks(&matrix, QUIET_ZONE, ink);
        let side = matrix.width() + 2 * QUIET_ZONE;
        assert_eq!(lines.len(), side.div_ceil(2));
        let (width, pixels) = expand(&lines, ink, 4);
        // Ink::Light draws light modules: on a dark terminal the code reads normally.
        let text = decode(width, |x, y| pixels.get(y).is_none_or(|row| row[x]));
        assert_eq!(text, URI, "{ink:?}");
    }
}

#[test]
fn the_quiet_zone_is_blank_and_text_too_long_is_refused() {
    let matrix = QrMatrix::encode("dail").unwrap();
    let lines = half_blocks(&matrix, QUIET_ZONE, Ink::Dark);
    assert!(lines[0].chars().all(|c| c == ' '));
    assert!(lines[1].chars().all(|c| c == ' '));
    assert!(!matrix.is_dark(matrix.width(), 0));
    assert_eq!(
        QrMatrix::encode(&"x".repeat(4000)).unwrap_err(),
        QrError::TooLong
    );
}
