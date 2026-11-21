//! QR codes (ADR 0017, SRV-QR1): the module matrix of a text, from the pure-Rust `qrcode` crate
//! without its renderers, and the half-block text the terminal and the TUI draw. The GUI paints
//! the matrix itself.
//!
//! **Error correction M** (about 15 % of the codewords may be lost). A pairing URI is short (about
//! 150 bytes), so M keeps the code at version 7 or so: modules stay large in a terminal, where a
//! character cell is the only pixel there is, and the code still survives a screen's glare and
//! moiré. L would be smaller but fragile on screens; Q and H grow the code by a third and more.

use qrcode::{EcLevel, QrCode};

/// The quiet zone the QR standard asks for around the code, in modules.
pub const QUIET_ZONE: usize = 4;

/// Why a text has no QR code.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum QrError {
    /// The text does not fit in the largest QR code at level M (2331 bytes).
    #[error("ERR461: the text is too long for a QR code")]
    TooLong,
}

/// The modules of a QR code: `width` × `width`, row by row, `true` for dark.
#[derive(Clone, PartialEq, Eq)]
pub struct QrMatrix {
    width: usize,
    dark: Vec<bool>,
}

impl std::fmt::Debug for QrMatrix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The modules spell out the text (a pairing code); only the size is shown.
        f.debug_struct("QrMatrix")
            .field("width", &self.width)
            .finish_non_exhaustive()
    }
}

impl QrMatrix {
    /// The QR code of `text` (byte mode as the encoder sees fit), at error correction level M.
    pub fn encode(text: &str) -> Result<Self, QrError> {
        let code = QrCode::with_error_correction_level(text.as_bytes(), EcLevel::M)
            .map_err(|_| QrError::TooLong)?;
        Ok(Self {
            width: code.width(),
            dark: code
                .to_colors()
                .into_iter()
                .map(|c| c == qrcode::Color::Dark)
                .collect(),
        })
    }

    /// The number of modules on a side, without the quiet zone.
    pub const fn width(&self) -> usize {
        self.width
    }

    /// Whether the module at column `x`, row `y` is dark. Outside the code (the quiet zone) every
    /// module is light.
    pub fn is_dark(&self, x: usize, y: usize) -> bool {
        x < self.width
            && y < self.width
            && self.dark.get(y * self.width + x).copied().unwrap_or(false)
    }

    /// Whether the module at `(x, y)` of the code framed by `quiet` light modules on each side is
    /// dark (coordinates count from the frame's corner).
    fn framed_dark(&self, quiet: usize, x: usize, y: usize) -> bool {
        match (x.checked_sub(quiet), y.checked_sub(quiet)) {
            (Some(x), Some(y)) => self.is_dark(x, y),
            _ => false,
        }
    }
}

/// Which modules the glyphs of [`half_blocks`] draw in the text's colour (its "ink").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ink {
    /// The glyphs are the dark modules; the background must be light. For text drawn in explicit
    /// colours, black on white: it scans whatever the terminal's theme. The CLI and the TUI use it.
    Dark,
    /// The glyphs are the light modules (and the quiet zone); the background is dark. For plain
    /// text on a terminal with a light foreground on a dark background, the common default
    /// (the CLI with `NO_COLOR`). On a light terminal it comes out inverted, which not every
    /// scanner reads.
    Light,
}

/// The code framed by `quiet` light modules, as lines of text: each character is one module wide
/// and two tall (a terminal cell is about twice as high as wide, so modules come out square),
/// drawn with `▀`, `▄`, `█` and spaces. Every line has the same number of characters; an odd
/// last row's lower half is quiet zone.
pub fn half_blocks(matrix: &QrMatrix, quiet: usize, ink: Ink) -> Vec<String> {
    let side = matrix.width() + 2 * quiet;
    let inked = |x: usize, y: usize| {
        let dark = y < side && matrix.framed_dark(quiet, x, y);
        match ink {
            Ink::Dark => dark,
            Ink::Light => y < side && !dark,
        }
    };
    (0..side.div_ceil(2))
        .map(|row| {
            let (top, bottom) = (row * 2, row * 2 + 1);
            (0..side)
                .map(|x| match (inked(x, top), inked(x, bottom)) {
                    (true, true) => '█',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    (false, false) => ' ',
                })
                .collect()
        })
        .collect()
}

/// The code framed by `quiet` light modules, as squares to paint: `(column, row)` of each dark
/// module in the frame's coordinates, and the frame's side in modules. A painter fills the whole
/// frame light (white, even in a dark theme) and then each of these dark (black).
pub fn dark_modules(matrix: &QrMatrix, quiet: usize) -> (usize, Vec<(usize, usize)>) {
    let side = matrix.width() + 2 * quiet;
    let dark = (0..side)
        .flat_map(|y| (0..side).map(move |x| (x, y)))
        .filter(|&(x, y)| matrix.framed_dark(quiet, x, y))
        .collect();
    (side, dark)
}
