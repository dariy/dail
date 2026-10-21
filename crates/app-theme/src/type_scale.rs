//! Type tokens: face, size, line height, weight and tracking.

use serde::{Deserialize, Serialize};

/// A typeface family. The fonts arrive with UI-2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Face {
    /// Geist.
    Sans,
    /// JetBrains Mono.
    Mono,
}

/// One text style.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TypeToken {
    pub face: Face,
    /// Points.
    pub size: f32,
    /// Points.
    pub line_height: f32,
    /// 400, 500 or 600.
    pub weight: u16,
    /// Letter spacing in em (×size for points).
    pub tracking: f32,
}

const fn token(face: Face, size: f32, line_height: f32, weight: u16, tracking: f32) -> TypeToken {
    TypeToken {
        face,
        size,
        line_height,
        weight,
        tracking,
    }
}

/// The type scale shared by the built-in themes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TypeScale {
    /// Thread subject.
    pub headline_xl: TypeToken,
    /// Page and dialog titles.
    pub headline_lg: TypeToken,
    /// Pane titles, sender in the card, palette input, brand.
    pub headline_md: TypeToken,
    /// Message and composer bodies.
    pub body_lg: TypeToken,
    /// Interface text, rows, nav rows, buttons (at 500).
    pub body_md: TypeToken,
    /// Snippets, secondary lines, tooltips.
    pub body_sm: TypeToken,
    /// Dates in the reader, code, field labels.
    pub label_md: TypeToken,
    /// Times in rows, captions (capitals), status.
    pub label_sm: TypeToken,
    /// Key caps, counters, chips.
    pub code_badge: TypeToken,
}

impl TypeScale {
    /// The scale of the design (§3.4 of the UI plan).
    pub const DEFAULT: Self = Self {
        headline_xl: token(Face::Sans, 28.0, 34.0, 600, -0.03),
        headline_lg: token(Face::Sans, 20.0, 26.0, 600, -0.02),
        headline_md: token(Face::Sans, 15.0, 20.0, 600, -0.01),
        body_lg: token(Face::Sans, 15.0, 24.0, 400, -0.01),
        body_md: token(Face::Sans, 13.0, 20.0, 400, 0.0),
        body_sm: token(Face::Sans, 12.0, 18.0, 400, 0.0),
        label_md: token(Face::Mono, 12.0, 16.0, 500, -0.01),
        label_sm: token(Face::Mono, 11.0, 14.0, 500, 0.02),
        code_badge: token(Face::Mono, 10.0, 12.0, 600, 0.04),
    };

    /// Every token, largest first.
    pub fn all(&self) -> [TypeToken; 9] {
        [
            self.headline_xl,
            self.headline_lg,
            self.headline_md,
            self.body_lg,
            self.body_md,
            self.body_sm,
            self.label_md,
            self.label_sm,
            self.code_badge,
        ]
    }
}
