//! Inline images of the open conversation: the `cid:` parts of a message, shown only for senders
//! the user lets show images (`show_images_from`). Nothing remote is ever loaded.
//!
//! The reader asks [`Images::get`] for each image it draws. A missing one is requested once
//! ([`Images::take_wanted`] → `External::LoadImage`); the app reads the part from the store and
//! decodes it off the UI thread ([`decode`]), and the result comes back through
//! [`Images::accept`].

use std::collections::HashMap;

use egui::{ColorImage, TextureHandle, TextureOptions};

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

/// Decodes an image part with the shared limits ([`app_sync::images::decode`]) into a
/// `ColorImage`. Untrusted input: anything else is an error.
pub fn decode(bytes: &[u8]) -> Result<ColorImage, String> {
    let image = app_sync::images::decode(bytes).map_err(|err| err.to_string())?;
    let size = [image.width as usize, image.height as usize];
    Ok(ColorImage::from_rgba_unmultiplied(size, &image.pixels))
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
        let huge = image::RgbaImage::new(app_sync::images::MAX_SIDE + 1, 1);
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
