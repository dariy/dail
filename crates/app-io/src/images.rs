//! Decoding untrusted images (inline `cid:` parts, attachment previews) with the limits every
//! front-end shares: PNG, JPEG, GIF (the first frame) or WebP, recognised by their bytes, at most
//! [`MAX_SIDE`] pixels per side (larger ones are decoded downscaled to fit) and [`MAX_ALLOC`]
//! bytes of decoder memory, turned upright by their EXIF orientation. Nothing remote is ever
//! loaded.

use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits, imageops::FilterType};

/// The largest image side returned; larger images are decoded downscaled to fit.
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

fn decode_error(err: impl std::fmt::Display) -> ImageError {
    ImageError::Decode(err.to_string())
}

/// Decodes an image part within the limits. Untrusted input: anything else is an error.
pub fn decode(bytes: &[u8]) -> Result<Rgba, ImageError> {
    let mut reader = ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(decode_error)?;
    let format = reader.format();
    if !matches!(
        format,
        Some(ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::Gif | ImageFormat::WebP)
    ) {
        return Err(ImageError::Format);
    }
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_ALLOC);
    reader.limits(limits.clone());
    let mut decoder = reader.into_decoder().map_err(decode_error)?;
    let orientation = decoder.orientation().map_err(decode_error)?;
    let (width, height) = decoder.dimensions();
    let fits = width <= MAX_SIDE && height <= MAX_SIDE;
    let mut image = if format == Some(ImageFormat::Jpeg) && !fits {
        drop(decoder);
        jpeg_scaled(bytes)?
    } else {
        // `into_decoder` skips the allocation check `ImageReader::decode` makes.
        limits
            .reserve(decoder.total_bytes())
            .map_err(decode_error)?;
        decoder.set_limits(limits).map_err(decode_error)?;
        let image = DynamicImage::from_decoder(decoder).map_err(decode_error)?;
        if fits {
            image
        } else {
            image.resize(MAX_SIDE, MAX_SIDE, FilterType::Triangle)
        }
    };
    image.apply_orientation(orientation);
    let image = image.to_rgba8();
    Ok(Rgba {
        width: image.width(),
        height: image.height(),
        pixels: image.into_raw(),
    })
}

/// A JPEG larger than [`MAX_SIDE`], decoded at the largest DCT scale (1, ½, ¼ or ⅛) that fits.
fn jpeg_scaled(bytes: &[u8]) -> Result<DynamicImage, ImageError> {
    use jpeg_decoder::{CodingProcess, Decoder, PixelFormat};
    let mut decoder = Decoder::new(bytes);
    decoder.set_max_decoding_buffer_size(usize::try_from(MAX_ALLOC).unwrap_or(usize::MAX));
    decoder.read_info().map_err(decode_error)?;
    let info = decoder
        .info()
        .ok_or_else(|| decode_error("no JPEG header"))?;
    let (w, h) = (u64::from(info.width), u64::from(info.height));
    let components: u64 = match info.pixel_format {
        PixelFormat::L8 => 1,
        PixelFormat::RGB24 => 3,
        other => return Err(decode_error(format!("unsupported JPEG pixels {other:?}"))),
    };
    if info.coding_process == CodingProcess::DctProgressive && w * h * components * 2 > MAX_ALLOC {
        return Err(decode_error("progressive JPEG too large"));
    }
    let side = u64::from(MAX_SIDE);
    let factor = [1u64, 2, 4, 8]
        .into_iter()
        .find(|f| w.div_ceil(*f) <= side && h.div_ceil(*f) <= side)
        .ok_or_else(|| decode_error("JPEG too large"))?;
    let want = |v: u64| u16::try_from(v.div_ceil(factor)).unwrap_or(u16::MAX);
    let (out_w, out_h) = decoder.scale(want(w), want(h)).map_err(decode_error)?;
    let pixels = decoder.decode().map_err(decode_error)?;
    let (out_w, out_h) = (u32::from(out_w), u32::from(out_h));
    let image = match info.pixel_format {
        PixelFormat::L8 => image::GrayImage::from_raw(out_w, out_h, pixels).map(DynamicImage::from),
        _ => image::RgbImage::from_raw(out_w, out_h, pixels).map(DynamicImage::from),
    };
    image.ok_or_else(|| decode_error("short JPEG data"))
}
