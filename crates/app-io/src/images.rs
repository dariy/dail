//! Decoding untrusted images (inline `cid:` parts, attachment previews) with the limits every
//! front-end shares: PNG, JPEG, GIF (the first frame) or WebP, recognised by their bytes, at most
//! [`MAX_SIDE`] pixels per side and [`MAX_ALLOC`] bytes of decoder memory. Nothing remote is
//! ever loaded.

use image::{ImageFormat, ImageReader, Limits};

/// The largest image decoded, per side (bigger ones are refused, not scaled).
pub const MAX_SIDE: u32 = 4096;
/// The most memory a decoder may use.
pub const MAX_ALLOC: u64 = 96 << 20;

/// A decoded image: unmultiplied RGBA, row by row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgba {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes.
    pub pixels: Vec<u8>,
}

/// Why an image was not decoded.
#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    /// The bytes are not one of the accepted formats.
    #[error("not a PNG, JPEG, GIF or WebP image")]
    Format,
    /// Unreadable, too large, or corrupt.
    #[error("{0}")]
    Decode(String),
}

/// Decodes an image part within the limits. Untrusted input: anything else is an error.
pub fn decode(bytes: &[u8]) -> Result<Rgba, ImageError> {
    let mut reader = ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|err| ImageError::Decode(err.to_string()))?;
    if !matches!(
        reader.format(),
        Some(ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::Gif | ImageFormat::WebP)
    ) {
        return Err(ImageError::Format);
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(MAX_ALLOC);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|err| ImageError::Decode(err.to_string()))?
        .to_rgba8();
    Ok(Rgba {
        width: image.width(),
        height: image.height(),
        pixels: image.into_raw(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(image: &image::RgbaImage) -> Vec<u8> {
        let mut out = Vec::new();
        image
            .write_to(&mut std::io::Cursor::new(&mut out), ImageFormat::Png)
            .unwrap();
        out
    }

    #[test]
    fn images_decode_within_limits() {
        let small = image::RgbaImage::from_raw(2, 1, vec![255, 0, 0, 255, 0, 0, 0, 0]).unwrap();
        let image = decode(&png(&small)).unwrap();
        assert_eq!((image.width, image.height), (2, 1));
        assert_eq!(image.pixels, [255, 0, 0, 255, 0, 0, 0, 0]);
        assert!(matches!(
            decode(b"<svg xmlns='http://www.w3.org/2000/svg'/>"),
            Err(ImageError::Format)
        ));
        assert!(decode(b"").is_err());
        let huge = image::RgbaImage::new(MAX_SIDE + 1, 1);
        assert!(decode(&png(&huge)).is_err(), "too wide");
    }
}
