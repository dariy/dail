//! Untrusted images: limits, downscaled decoding of large ones, EXIF orientation.

// reason: test helpers build fixtures; a failure is the test failing.
#![allow(clippy::unwrap_used)]

use app_io::images::{ImageError, MAX_SIDE, decode};
use image::{ImageFormat, RgbImage, RgbaImage, codecs::jpeg::JpegEncoder};

fn png(image: &RgbaImage) -> Vec<u8> {
    let mut out = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut out), ImageFormat::Png)
        .unwrap();
    out
}

fn jpeg(width: u32, height: u32) -> Vec<u8> {
    // Smooth content: a gradient compresses small and decodes the same everywhere.
    let image = RgbImage::from_fn(width, height, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
    });
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 60)
        .encode_image(&image)
        .unwrap();
    out
}

/// Inserts an EXIF APP1 segment with the given orientation after the JPEG's SOI marker.
fn with_orientation(jpeg: &[u8], orientation: u16) -> Vec<u8> {
    let mut exif = b"Exif\0\0MM\0\x2a\0\0\0\x08\0\x01\x01\x12\0\x03\0\0\0\x01".to_vec();
    exif.extend_from_slice(&orientation.to_be_bytes());
    exif.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
    let len = u16::try_from(exif.len() + 2).unwrap();
    let mut out = jpeg[..2].to_vec();
    out.extend_from_slice(&[0xFF, 0xE1]);
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&exif);
    out.extend_from_slice(&jpeg[2..]);
    out
}

#[test]
fn images_decode_within_limits() {
    let small = RgbaImage::from_raw(2, 1, vec![255, 0, 0, 255, 0, 0, 0, 0]).unwrap();
    let image = decode(&png(&small)).unwrap();
    assert_eq!((image.width, image.height), (2, 1));
    assert_eq!(image.pixels, [255, 0, 0, 255, 0, 0, 0, 0]);
    assert!(matches!(
        decode(b"<svg xmlns='http://www.w3.org/2000/svg'/>"),
        Err(ImageError::Format)
    ));
    assert!(decode(b"").is_err());
}

#[test]
fn a_6000x4000_jpeg_decodes_to_at_most_4096_a_side() {
    let image = decode(&jpeg(6000, 4000)).unwrap();
    assert_eq!((image.width, image.height), (3000, 2000), "the ½ DCT scale");
    assert_eq!(image.pixels.len(), 3000 * 2000 * 4);
}

#[test]
fn exif_orientation_6_comes_out_upright() {
    let small = decode(&with_orientation(&jpeg(40, 20), 6)).unwrap();
    assert_eq!((small.width, small.height), (20, 40), "the image path");
    let large = decode(&with_orientation(&jpeg(MAX_SIDE + 4, 8), 6)).unwrap();
    assert_eq!(
        (large.width, large.height),
        (4, 2050),
        "the scaled JPEG path"
    );
}

#[test]
fn oversized_png_is_scaled_to_fit() {
    let image = decode(&png(&RgbaImage::new(MAX_SIDE * 2, 2))).unwrap();
    assert_eq!((image.width, image.height), (MAX_SIDE, 1));
}

#[test]
fn a_jpeg_too_large_at_one_eighth_is_refused() {
    let mut bytes = jpeg(16, 16);
    // The SOF0 frame header holds the height and width as big-endian u16 after its precision.
    let sof = bytes.windows(2).position(|w| w == [0xFF, 0xC0]).unwrap();
    bytes[sof + 5..sof + 9].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
    assert!(decode(&bytes).is_err());
}

#[test]
fn a_crafted_png_header_is_refused_without_allocating() {
    let mut bytes = png(&RgbaImage::new(1, 1));
    // IHDR's width and height follow the 8-byte signature, the length and the chunk type.
    bytes[16..24].copy_from_slice(&[0, 0, 0xFF, 0xFF, 0, 0, 0xFF, 0xFF]);
    assert!(decode(&bytes).is_err());
}
