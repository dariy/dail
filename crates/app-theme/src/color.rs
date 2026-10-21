//! Colours in sRGB, blending and the WCAG 2.x contrast ratio.

use serde::{Deserialize, Serialize};

/// An sRGB colour with straight (not premultiplied) alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// The opaque colour `0xRRGGBB`.
pub const fn hex(rgb: u32) -> Rgba {
    Rgba {
        r: (rgb >> 16) as u8,
        g: (rgb >> 8) as u8,
        b: rgb as u8,
        a: 0xFF,
    }
}

impl Rgba {
    /// The same colour at `alpha` (0–255).
    pub const fn with_alpha(self, alpha: u8) -> Self {
        Self { a: alpha, ..self }
    }

    /// This colour painted over the opaque `under`: the colour the eye sees.
    pub fn over(self, under: Self) -> Self {
        let a = f32::from(self.a) / 255.0;
        let mix = |top: u8, bottom: u8| {
            (f32::from(top) * a + f32::from(bottom) * (1.0 - a)).round() as u8
        };
        Self {
            r: mix(self.r, under.r),
            g: mix(self.g, under.g),
            b: mix(self.b, under.b),
            a: 0xFF,
        }
    }

    /// WCAG 2.x relative luminance of the opaque colour, 0 (black) to 1 (white).
    pub fn luminance(self) -> f64 {
        let channel = |c: u8| {
            let c = f64::from(c) / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b)
    }
}

/// WCAG 2.x contrast ratio of `fg` on the opaque `bg`, 1 to 21. A translucent `fg` is blended first.
pub fn contrast(fg: Rgba, bg: Rgba) -> f64 {
    let (a, b) = (fg.over(bg).luminance(), bg.luminance());
    let (light, dark) = if a > b { (a, b) } else { (b, a) };
    (light + 0.05) / (dark + 0.05)
}
