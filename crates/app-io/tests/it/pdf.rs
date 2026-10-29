//! Untrusted PDFs (ADR 0012): pages and tiles at the asked size, limits, and malformed files that
//! end in an error within a time bound, never a panic or a hang.

// reason: test helpers build fixtures; a failure is the test failing.
#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::time::Duration;

use app_io::images::{MAX_SIDE, Rgba};
use app_io::pdf::{MAX_ZOOM_WIDTH, PageSize, PdfError, TILE_MAX, Tile, pages, render};
use hayro::hayro_syntax::PdfData;

/// A PDF of the given objects (numbered from 1; object 1 is the catalog), with a correct xref.
fn pdf(objects: &[String]) -> Vec<u8> {
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

fn stream(dict: &str, data: &[u8]) -> String {
    format!(
        "<< {dict} /Length {} >>\nstream\n{}\nendstream",
        data.len(),
        String::from_utf8_lossy(data)
    )
}

/// Pages with the given page dictionaries' extra entries, each drawing `content`.
fn document(pages: &[(&str, &str)]) -> Vec<u8> {
    let n = pages.len();
    let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", 3 + 2 * i)).collect();
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        format!("<< /Type /Pages /Kids [{}] /Count {n} >>", kids.join(" ")),
    ];
    let font = 3 + 2 * n;
    for (i, (entries, content)) in pages.iter().enumerate() {
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R {entries} /Contents {} 0 R /Resources << /Font << /F1 {font} 0 R >> >> >>",
            4 + 2 * i
        ));
        objects.push(stream("", content.as_bytes()));
    }
    objects.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string());
    pdf(&objects)
}

const INK: &str = "0 0 1 rg 50 50 200 100 re f BT /F1 24 Tf 60 700 Td (Hello dail) Tj ET";

fn a4() -> PdfData {
    data(document(&[("/MediaBox [0 0 595 842]", INK)]))
}

fn data(bytes: Vec<u8>) -> PdfData {
    PdfData::from(Arc::new(bytes))
}

fn inked(image: &Rgba) -> usize {
    image
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[0] < 200 || p[1] < 200 || p[2] < 200)
        .count()
}

#[test]
fn a_generated_page_renders_at_the_asked_width_with_ink() {
    let image = render(&a4(), 0, 1190, None).unwrap();
    assert_eq!((image.width, image.height), (1190, 1684));
    assert_eq!(image.pixels.len(), 1190 * 1684 * 4);
    assert!(inked(&image) > 1000, "the rectangle and the text");
    assert_eq!(&image.pixels[..4], &[255, 255, 255, 255], "white paper");
}

#[test]
fn a_tile_matches_the_same_region_of_the_page() {
    let doc = a4();
    let page = render(&doc, 0, 1190, None).unwrap();
    let tile = Tile {
        x: 80,
        y: 1350,
        w: 300,
        h: 200,
    };
    let part = render(&doc, 0, 1190, Some(tile)).unwrap();
    assert_eq!((part.width, part.height), (300, 200));
    assert!(inked(&part) > 100, "the rectangle is in this tile");
    let mut differing = 0;
    for row in 0..tile.h {
        for col in 0..tile.w {
            let a = ((row * tile.w + col) * 4) as usize;
            let b = (((tile.y + row) * page.width + tile.x + col) * 4) as usize;
            let delta = (0..4)
                .map(|i| part.pixels[a + i].abs_diff(page.pixels[b + i]))
                .max();
            differing += usize::from(delta > Some(8));
        }
    }
    assert!(differing < 300, "{differing} pixels differ");
}

#[test]
fn page_sizes_are_points_with_rotation() {
    let doc = data(document(&[
        ("/MediaBox [0 0 595 842]", INK),
        ("/MediaBox [0 0 595 842] /Rotate 90", INK),
        (
            "/MediaBox [0 0 842 595]",
            "BT /F1 4 Tf 10 10 Td (tiny) Tj ET",
        ),
    ]));
    let sizes = pages(&doc).unwrap();
    let size = |width, height| PageSize { width, height };
    assert_eq!(
        sizes,
        [size(595.0, 842.0), size(842.0, 595.0), size(842.0, 595.0)]
    );
    let rotated = render(&doc, 1, 842, None).unwrap();
    assert_eq!((rotated.width, rotated.height), (842, 595));
}

#[test]
fn widths_tiles_and_tall_pages_respect_the_limits() {
    let doc = a4();
    assert_eq!(render(&doc, 0, 0, None), Err(PdfError::PageSize));
    assert_eq!(render(&doc, 0, MAX_SIDE + 1, None), Err(PdfError::PageSize));
    let tile = |x, y, w, h| Some(Tile { x, y, w, h });
    assert_eq!(
        render(&doc, 0, MAX_ZOOM_WIDTH + 1, tile(0, 0, 8, 8)),
        Err(PdfError::PageSize)
    );
    assert_eq!(
        render(&doc, 0, 1000, tile(0, 0, TILE_MAX + 1, 8)),
        Err(PdfError::Tile)
    );
    assert_eq!(render(&doc, 0, 1000, tile(0, 0, 0, 8)), Err(PdfError::Tile));
    assert_eq!(
        render(&doc, 0, 1000, tile(990, 0, 20, 8)),
        Err(PdfError::Tile)
    );
    assert_eq!(
        render(&doc, 0, 1000, tile(0, 1410, 8, 8)),
        Err(PdfError::Tile)
    );
    assert_eq!(
        render(&doc, 0, 1000, tile(u32::MAX, 0, 8, 8)),
        Err(PdfError::Tile)
    );
    let deep = render(
        &doc,
        0,
        MAX_ZOOM_WIDTH,
        tile(8000, 11000, TILE_MAX, TILE_MAX),
    )
    .unwrap();
    assert_eq!((deep.width, deep.height), (TILE_MAX, TILE_MAX));
    // A strip 10 pt wide and 5000 pt tall comes out within MAX_SIDE on both sides.
    let tall = data(document(&[("/MediaBox [0 0 10 5000]", INK)]));
    let image = render(&tall, 0, 4000, None).unwrap();
    assert_eq!(image.height, MAX_SIDE);
    assert!(image.width >= 1 && image.width <= 9);
}

#[test]
fn a_page_past_the_end_is_an_error() {
    assert_eq!(render(&a4(), 1, 100, None), Err(PdfError::NoPage(1)));
}

#[test]
fn too_large_and_too_many_pages_are_refused() {
    let mut big = document(&[("/MediaBox [0 0 595 842]", INK)]);
    big.resize(app_io::pdf::MAX_BYTES + 1, b' ');
    assert_eq!(pages(&data(big)), Err(PdfError::TooLarge));
    let many: Vec<(&str, &str)> = vec![("/MediaBox [0 0 10 10]", ""); app_io::pdf::MAX_PAGES + 1];
    assert_eq!(pages(&data(document(&many))), Err(PdfError::TooManyPages));
}

#[test]
fn truncated_pdfs_are_errors_not_panics() {
    let bytes = document(&[("/MediaBox [0 0 595 842]", INK)]);
    for end in (0..bytes.len()).step_by(7) {
        let doc = data(bytes[..end].to_vec());
        if let Ok(sizes) = pages(&doc) {
            for page in 0..sizes.len() {
                let _ = render(&doc, page, 200, None);
            }
        }
    }
}

/// Runs `pages` and renders every page at 600 px in a thread, which must end within 5 s.
fn ends_in_time(name: &str, bytes: Vec<u8>) {
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let doc = data(bytes);
        let result = pages(&doc).map(|sizes| {
            (0..sizes.len())
                .map(|page| render(&doc, page, 600, None).map(|_| ()))
                .collect::<Vec<_>>()
        });
        let _ = send.send(format!("{result:?}"));
    });
    let result = receive.recv_timeout(Duration::from_secs(5));
    assert!(result.is_ok(), "{name}: no answer within 5 s ({result:?})");
}

#[test]
fn malformed_pdfs_end_within_a_time_bound() {
    ends_in_time("empty", Vec::new());
    ends_in_time("garbage", b"\x00\xff garbage %%EOF".repeat(100));
    ends_in_time("header only", b"%PDF-1.7\n".to_vec());
    let mut broken_xref = document(&[("/MediaBox [0 0 595 842]", INK)]);
    let at = broken_xref.windows(4).position(|w| w == b"xref").unwrap();
    broken_xref[at + 7..at + 20].fill(b'9');
    ends_in_time("broken xref", broken_xref);
    ends_in_time(
        "an XObject that draws itself",
        pdf(&[
            "<< /Type /Catalog /Pages 2 0 R >>".into(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R /Resources << /XObject << /X 5 0 R >> >> >>".into(),
            stream("", b"/X Do"),
            stream(
                "/Type /XObject /Subtype /Form /BBox [0 0 100 100] /Resources << /XObject << /X 5 0 R >> >>",
                b"/X Do",
            ),
        ]),
    );
    ends_in_time(
        "a /Kids loop",
        pdf(&[
            "<< /Type /Catalog /Pages 2 0 R >>".into(),
            "<< /Type /Pages /Kids [2 0 R 3 0 R] /Count 2 >>".into(),
            "<< /Type /Pages /Parent 2 0 R /Kids [2 0 R] /Count 1 >>".into(),
        ]),
    );
    ends_in_time("a zero MediaBox", document(&[("/MediaBox [0 0 0 0]", INK)]));
    ends_in_time(
        "a huge MediaBox",
        document(&[("/MediaBox [0 0 1e30 1e30]", INK)]),
    );
}

fn page(resources: &str, content: &[u8]) -> Vec<String> {
    vec![
        "<< /Type /Catalog /Pages 2 0 R >>".into(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R /Resources {resources} >>"
        ),
        stream("", content),
    ]
}

fn image(dict: &str) -> Vec<u8> {
    let mut objects = page("<< /XObject << /I 5 0 R >> >>", b"100 0 0 100 0 0 cm /I Do");
    objects.push(stream(
        &format!("/Type /XObject /Subtype /Image {dict}"),
        b"\x00\x00\x00\x00",
    ));
    pdf(&objects)
}

// pdfcraft's fuzz regressions, generated: each must end in time without aborting the process.

#[test]
fn a_long_dict_run_in_a_cmap_does_not_abort() {
    // The vendored hayro-postscript fix (vendor/README.md).
    let mut objects = page(
        "<< /Font << /F1 5 0 R >> >>",
        b"BT /F1 12 Tf 10 10 Td <0001> Tj ET",
    );
    objects.push(
        "<< /Type /Font /Subtype /Type0 /BaseFont /X /Encoding 6 0 R /DescendantFonts [7 0 R] >>"
            .into(),
    );
    let cmap = [
        b"/CIDInit /ProcSet findresource begin 12 dict begin begincmap ".to_vec(),
        b"<<".repeat(400_000),
        b">>".repeat(400_000),
        b" endcmap end end".to_vec(),
    ]
    .concat();
    objects.push(stream("/Type /CMap /CMapName /X", &cmap));
    objects.push(
        "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /X /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> >>"
            .into(),
    );
    ends_in_time("a long << run in a CMap", pdf(&objects));
}

/// Risk (ADR 0012): hayro-ccitt allocates `Columns × 8` bytes up front, so this aborts the process
/// on a failed 32 GiB allocation. Unbounded at the pinned commit; run by hand after a hayro bump.
#[test]
#[ignore = "aborts: hayro-ccitt allocation is unbounded (ADR 0012)"]
fn ccitt_huge_columns_allocation_aborts() {
    ends_in_time(
        "CCITT /Columns 4294967295",
        image(
            "/Width 8 /Height 8 /BitsPerComponent 1 /ColorSpace /DeviceGray /Filter /CCITTFaxDecode /DecodeParms << /Columns 4294967295 /K -1 >>",
        ),
    );
}

#[test]
fn a_huge_predictor_columns_does_not_abort() {
    ends_in_time(
        "a huge predictor /Columns",
        image(
            "/Width 8 /Height 8 /BitsPerComponent 8 /ColorSpace /DeviceGray /Filter /FlateDecode /DecodeParms << /Predictor 12 /Columns 4294967295 >>",
        ),
    );
}

/// Risk (ADR 0012): a 2³¹ × 2³¹ image is refused only after seconds of work (3.8 s optimized, past
/// 5 s in a debug build); the host's 10 s limit bounds it.
#[test]
#[ignore = "slow: hayro takes seconds on a huge image header (ADR 0012)"]
fn a_huge_image_takes_seconds() {
    ends_in_time(
        "a huge /W image",
        image("/Width 2147483647 /Height 2147483647 /BitsPerComponent 8 /ColorSpace /DeviceRGB"),
    );
}

#[test]
fn a_huge_jbig2_page_does_not_abort() {
    ends_in_time(
        "a huge JBIG2 page",
        image(
            "/Width 8 /Height 8 /BitsPerComponent 1 /ColorSpace /DeviceGray /Filter /JBIG2Decode",
        ),
    );
}

/// A measurement for ADR 0012, ignored by default:
/// `cargo test -p app-io --release --test it pdf::render_times -- --ignored --nocapture`.
#[test]
#[ignore = "a measurement, not a check of behaviour"]
#[allow(clippy::print_stderr)] // reason: a measurement prints its results.
fn render_times() {
    let doc = a4();
    for (what, width, tile) in [
        ("page at 1080 px", 1080, None),
        ("page at 4096 px", MAX_SIDE, None),
        (
            "tile 1024² at 16384 px",
            MAX_ZOOM_WIDTH,
            Some(Tile {
                x: 4000,
                y: 8000,
                w: TILE_MAX,
                h: TILE_MAX,
            }),
        ),
    ] {
        let start = std::time::Instant::now();
        let image = render(&doc, 0, width, tile).unwrap();
        eprintln!(
            "{what}: {:?} ({}×{})",
            start.elapsed(),
            image.width,
            image.height
        );
    }
}
