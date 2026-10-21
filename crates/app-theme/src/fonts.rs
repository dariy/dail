//! The bundled typefaces (UI-2): Geist and JetBrains Mono, variable in `wght`, with their licences.
//! The front-end registers each font once per weight in [`WEIGHTS`] under [`family`].

use crate::type_scale::Face;

/// A bundled font file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontFile {
    /// A stable name for the font's bytes, e.g. `"Geist"`.
    pub name: &'static str,
    /// The face it provides.
    pub face: Face,
    /// The `.ttf` file.
    pub bytes: &'static [u8],
    /// Its variation axes.
    pub axes: &'static [&'static str],
    /// The licence text that must ship with it.
    pub license: &'static str,
    /// File name for the licence text in a package.
    pub license_file: &'static str,
}

/// Geist (OFL-1.1).
pub const GEIST: FontFile = FontFile {
    name: "Geist",
    face: Face::Sans,
    bytes: include_bytes!("../assets/fonts/Geist[wght].ttf"),
    axes: &["wght"],
    license: include_str!("../assets/fonts/OFL-Geist.txt"),
    license_file: "OFL-Geist.txt",
};

/// JetBrains Mono (OFL-1.1).
pub const JETBRAINS_MONO: FontFile = FontFile {
    name: "JetBrains Mono",
    face: Face::Mono,
    bytes: include_bytes!("../assets/fonts/JetBrainsMono[wght].ttf"),
    axes: &["wght"],
    license: include_str!("../assets/fonts/OFL-JetBrainsMono.txt"),
    license_file: "OFL-JetBrainsMono.txt",
};

/// The weights the type scale uses.
pub const WEIGHTS: [u16; 3] = [400, 500, 600];

/// Every bundled text font.
pub const fn fonts() -> [FontFile; 2] {
    [GEIST, JETBRAINS_MONO]
}

/// The font of `face`.
pub const fn file(face: Face) -> FontFile {
    match face {
        Face::Sans => GEIST,
        Face::Mono => JETBRAINS_MONO,
    }
}

/// The weight in [`WEIGHTS`] nearest to `weight`.
pub fn nearest_weight(weight: u16) -> u16 {
    WEIGHTS
        .into_iter()
        .min_by_key(|w| w.abs_diff(weight))
        .unwrap_or(400)
}

/// The family name a front-end registers `face` at `weight` under: `sans`, `sans-500`,
/// `sans-600`, `mono`, `mono-500`, `mono-600`. Weights snap to [`WEIGHTS`].
pub fn family(face: Face, weight: u16) -> &'static str {
    match (face, nearest_weight(weight)) {
        (Face::Sans, 500) => "sans-500",
        (Face::Sans, 600) => "sans-600",
        (Face::Sans, _) => "sans",
        (Face::Mono, 500) => "mono-500",
        (Face::Mono, 600) => "mono-600",
        (Face::Mono, _) => "mono",
    }
}
