//! Inline images of the open conversation: the `cid:` parts of a message, shown only for senders
//! the user lets show images (`show_images_from`). Nothing remote is ever loaded.
//!
//! The reader asks [`Images::get`] for each image it draws. A missing one is requested once
//! ([`Images::take_wanted`] → `External::LoadImage`); the app reads the part from the store and
//! decodes it off the UI thread ([`decode`]), and the result comes back through
//! [`Images::accept`].

use std::collections::HashMap;

use egui::{ColorImage, TextureHandle, TextureOptions};

/// The largest image decoded, per side (bigger ones are refused, not scaled).
const MAX_SIDE: u32 = 4096;
/// The most memory a decoder may use.
const MAX_ALLOC: u64 = 96 << 20;
/// Images kept at most; the cache starts over when it is full.
const MAX_CACHED: usize = 64;

/// An inline image: the message body's blob and the part's `Content-ID`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageKey {
    /// The body's blob ID (`BodyState::Fetched`).
    pub blob: String,
    /// The part's `Content-ID`.
    pub cid: String,
}

/// What the reader can draw for an image.
#[derive(Clone)]
pub enum Slot {
    /// Being read and decoded.
    Loading,
    /// Ready to draw.
    Ready(TextureHandle),
    /// Missing or not decodable: the alt text stands in.
    Failed(String),
}

/// The images of the open conversation.
#[derive(Default)]
pub struct Images {
    slots: HashMap<ImageKey, Slot>,
    wanted: Vec<ImageKey>,
}

impl Images {
    /// The image, requesting it if it isn't known yet.
    pub fn get(&mut self, key: &ImageKey) -> Slot {
        if let Some(slot) = self.slots.get(key) {
            return slot.clone();
        }
        if self.slots.len() >= MAX_CACHED {
            self.slots.clear();
        }
        self.slots.insert(key.clone(), Slot::Loading);
        self.wanted.push(key.clone());
        Slot::Loading
    }

    /// The images to load, each asked for once.
    pub fn take_wanted(&mut self) -> Vec<ImageKey> {
        std::mem::take(&mut self.wanted)
    }

    /// Takes a loaded image (or its failure) into the cache as a texture.
    pub fn accept(
        &mut self,
        ctx: &egui::Context,
        key: ImageKey,
        image: Result<ColorImage, String>,
    ) {
        let slot = match image {
            Ok(image) => Slot::Ready(ctx.load_texture(
                format!("cid:{}:{}", key.blob, key.cid),
                image,
                TextureOptions::LINEAR,
            )),
            Err(error) => Slot::Failed(error),
        };
        self.slots.insert(key, slot);
    }
}

/// Decodes an image part: PNG, JPEG, GIF (the first frame) or WebP, recognised by its bytes,
/// at most 4096 pixels per side. Untrusted input: anything else is an error.
pub fn decode(bytes: &[u8]) -> Result<ColorImage, String> {
    use image::{ImageFormat, ImageReader, Limits};
    let reader = ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|err| err.to_string())?;
    if !matches!(
        reader.format(),
        Some(ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::Gif | ImageFormat::WebP)
    ) {
        return Err("not a PNG, JPEG, GIF or WebP image".to_owned());
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(MAX_ALLOC);
    let mut reader = reader;
    reader.limits(limits);
    let image = reader.decode().map_err(|err| err.to_string())?.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    Ok(ColorImage::from_rgba_unmultiplied(size, image.as_raw()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 2×1 PNG: one red and one transparent pixel.
    fn png() -> Vec<u8> {
        let mut out = Vec::new();
        let image = image::RgbaImage::from_raw(2, 1, vec![255, 0, 0, 255, 0, 0, 0, 0]).unwrap();
        image
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    #[test]
    fn images_decode_within_limits() {
        let image = decode(&png()).unwrap();
        assert_eq!(image.size, [2, 1]);
        assert!(decode(b"<svg xmlns='http://www.w3.org/2000/svg'/>").is_err());
        assert!(decode(b"").is_err());
        let huge = image::RgbaImage::new(MAX_SIDE + 1, 1);
        let mut bytes = Vec::new();
        huge.write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
        assert!(decode(&bytes).is_err(), "too wide");
    }

    #[test]
    fn each_image_is_asked_for_once() {
        let mut images = Images::default();
        let key = ImageKey {
            blob: "b".into(),
            cid: "logo".into(),
        };
        assert!(matches!(images.get(&key), Slot::Loading));
        assert!(matches!(images.get(&key), Slot::Loading));
        assert_eq!(images.take_wanted(), std::slice::from_ref(&key));
        assert!(images.take_wanted().is_empty());
        let ctx = egui::Context::default();
        images.accept(&ctx, key.clone(), decode(&png()));
        assert!(matches!(images.get(&key), Slot::Ready(_)));
    }
}
