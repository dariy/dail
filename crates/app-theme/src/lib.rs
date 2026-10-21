//! Design tokens for the dail front-ends (ADR 0008): colour roles, type scale, metrics, the built-in
//! themes and their contrast audit. No toolkit: the egui adapter in `app-gui` converts a [`Theme`]
//! into egui types.
//!
//! Three layers: primitives (greys, sizes) → roles per theme ([`Surface`]) → components, which read
//! roles and metrics only and never a hex value or a pixel number.

pub mod audit;
pub mod builtin;
pub mod color;
pub mod fonts;
pub mod metrics;
pub mod surface;
pub mod theme;
pub mod type_scale;

pub use audit::{Problem, audit};
pub use color::{Rgba, contrast, hex};
pub use fonts::FontFile;
pub use metrics::Metrics;
pub use surface::Surface;
pub use theme::{Appearance, Theme};
pub use type_scale::{Face, TypeScale, TypeToken};
