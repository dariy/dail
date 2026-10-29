//! Rasterizing untrusted PDF pages with `hayro` (ADR 0012): the page sizes, and a page or a tile of
//! it at a given width as RGBA, the format of [`crate::images`]. Pure Rust; within the limits
//! below. Annotations are drawn as their printed appearance; links, forms, JavaScript and remote
//! content are never acted on.

use std::panic::{AssertUnwindSafe, catch_unwind};

use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_interpret::util::TransformExt;
use hayro::hayro_syntax::Pdf;
/// A PDF's bytes, shared rather than copied between calls.
pub use hayro::hayro_syntax::PdfData;
use hayro::kurbo::Affine;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::vello_cpu::{Pixmap, RasterizerSettings, RenderContext, Resources, TargetInit};
use hayro::{RenderCache, RenderSettings, render_into};

use crate::images::{MAX_SIDE, Rgba};

/// The largest PDF opened.
pub const MAX_BYTES: usize = 32 << 20;
/// The most pages a PDF opened may have.
pub const MAX_PAGES: usize = 1000;
/// The widest virtual page a tile is cut from (the deepest zoom).
pub const MAX_ZOOM_WIDTH: u32 = 16384;
/// The largest tile side.
pub const TILE_MAX: u32 = 1024;

/// A page's size in points as shown (after its rotation and crop box).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageSize {
    /// Width in points.
    pub width: f32,
    /// Height in points.
    pub height: f32,
}

/// A rectangle of a page rendered at a width, in that rendering's pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tile {
    /// Left edge.
    pub x: u32,
    /// Top edge.
    pub y: u32,
    /// Width.
    pub w: u32,
    /// Height.
    pub h: u32,
}

/// Why a PDF or a page was not rendered.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PdfError {
    /// More than [`MAX_BYTES`].
    #[error("the PDF is larger than {MAX_BYTES} bytes")]
    TooLarge,
    /// More than [`MAX_PAGES`].
    #[error("the PDF has more than {MAX_PAGES} pages")]
    TooManyPages,
    /// Not a PDF hayro can read (corrupt, encrypted, or not a PDF).
    #[error("unreadable PDF: {0}")]
    Unreadable(String),
    /// No page with this index.
    #[error("no page {0}")]
    NoPage(usize),
    /// The page's size is not finite or below one point, or the width is out of range.
    #[error("the page size is out of range")]
    PageSize,
    /// The tile is empty, larger than [`TILE_MAX`] or outside the page.
    #[error("the tile is out of range")]
    Tile,
    /// The renderer panicked.
    #[error("the PDF renderer failed")]
    Panicked,
}

fn open(data: &PdfData) -> Result<Pdf, PdfError> {
    if data.as_ref().len() > MAX_BYTES {
        return Err(PdfError::TooLarge);
    }
    let pdf = catch_unwind(AssertUnwindSafe(|| Pdf::new(data.clone())))
        .map_err(|_| PdfError::Panicked)?
        .map_err(|err| PdfError::Unreadable(format!("{err:?}")))?;
    if pdf.pages().len() > MAX_PAGES {
        return Err(PdfError::TooManyPages);
    }
    Ok(pdf)
}

fn size((width, height): (f32, f32)) -> Result<PageSize, PdfError> {
    if width.is_finite() && height.is_finite() && width >= 1.0 && height >= 1.0 {
        Ok(PageSize { width, height })
    } else {
        Err(PdfError::PageSize)
    }
}

/// The pages' sizes, in points as shown. A page of a size out of range is an error.
pub fn pages(data: &PdfData) -> Result<Vec<PageSize>, PdfError> {
    let pdf = open(data)?;
    catch_unwind(AssertUnwindSafe(|| {
        pdf.pages()
            .iter()
            .map(|page| size(page.render_dimensions()))
            .collect()
    }))
    .map_err(|_| PdfError::Panicked)?
}

/// Renders page `page` (from 0) at `width` pixels on white. Without a tile, the whole page,
/// with both sides from 1 to [`MAX_SIDE`] (a page too tall for `width` comes out narrower).
/// With a tile, that rectangle of the page rendered at `width` (at most [`MAX_ZOOM_WIDTH`]).
pub fn render(
    data: &PdfData,
    page: usize,
    width: u32,
    tile: Option<Tile>,
) -> Result<Rgba, PdfError> {
    let pdf = open(data)?;
    catch_unwind(AssertUnwindSafe(|| render_page(&pdf, page, width, tile)))
        .map_err(|_| PdfError::Panicked)?
}

fn render_page(pdf: &Pdf, index: usize, width: u32, tile: Option<Tile>) -> Result<Rgba, PdfError> {
    let page = pdf.pages().get(index).ok_or(PdfError::NoPage(index))?;
    let points = size(page.render_dimensions())?;
    let max_width = if tile.is_some() {
        MAX_ZOOM_WIDTH
    } else {
        MAX_SIDE
    };
    if width == 0 || width > max_width {
        return Err(PdfError::PageSize);
    }
    let fit = |pixels: f64| pixels.round().clamp(1.0, f64::from(u32::MAX)) as u32;
    let mut scale = f64::from(width) / f64::from(points.width);
    let full_height = fit(f64::from(points.height) * scale);
    let (x, y, w, h) = match tile {
        Some(tile) => {
            let inside = tile.w > 0
                && tile.h > 0
                && tile.w <= TILE_MAX
                && tile.h <= TILE_MAX
                && tile.x.checked_add(tile.w).is_some_and(|r| r <= width)
                && tile.y.checked_add(tile.h).is_some_and(|b| b <= full_height);
            if !inside {
                return Err(PdfError::Tile);
            }
            (tile.x, tile.y, tile.w, tile.h)
        }
        None => {
            if full_height > MAX_SIDE {
                scale = f64::from(MAX_SIDE) / f64::from(points.height);
            }
            let w = fit(f64::from(points.width) * scale).min(MAX_SIDE);
            let h = fit(f64::from(points.height) * scale).min(MAX_SIDE);
            (0, 0, w, h)
        }
    };
    let (w16, h16) = (
        u16::try_from(w).map_err(|_| PdfError::PageSize)?,
        u16::try_from(h).map_err(|_| PdfError::PageSize)?,
    );
    let transform = Affine::translate((-f64::from(x), -f64::from(y)))
        * Affine::scale(scale)
        * page.initial_transform(true).to_kurbo();
    let settings = InterpreterSettings {
        warning_sink: std::sync::Arc::new(|warning| tracing::debug!(?warning, "pdf")),
        ..InterpreterSettings::default()
    };
    let mut ctx = RenderContext::new(w16, h16);
    render_into(
        page,
        &RenderCache::new(),
        &settings,
        &RenderSettings::default(),
        &mut ctx,
        transform,
    );
    ctx.flush();
    let mut pixmap = Pixmap::new(w16, h16);
    ctx.render_with(
        &mut pixmap,
        &mut Resources::default(),
        RasterizerSettings {
            target_init: TargetInit::Clear(WHITE),
            ..Default::default()
        },
    );
    // On an opaque white background every pixel is opaque, so these bytes are unmultiplied too.
    Ok(Rgba {
        width: w,
        height: h,
        pixels: pixmap.data_as_u8_slice().to_vec(),
    })
}
