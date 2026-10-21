//! The bundled fonts (UI-2): six families, egui's fonts as fallbacks, weights by token.

use crate::support::{gui, synced_state};
use app_gui::theme;
use app_theme::TypeScale;
use egui::{FontFamily, FontId, TextStyle};

#[test]
fn fonts_are_installed_with_fallbacks() {
    let mut h = gui(synced_state());
    h.run();
    let families = h.ctx.fonts(|f| f.families());
    for name in [
        "sans", "sans-500", "sans-600", "mono", "mono-500", "mono-600",
    ] {
        assert!(
            families.contains(&FontFamily::Name(name.into())),
            "{name} in {families:?}"
        );
    }
    let defs = h.ctx.fonts(|f| f.definitions().clone());
    assert_eq!(
        defs.families[&FontFamily::Proportional]
            .first()
            .map(String::as_str),
        Some("Geist-400")
    );
    assert_eq!(
        defs.families[&FontFamily::Monospace]
            .first()
            .map(String::as_str),
        Some("JetBrains Mono-400")
    );
    // Latin, Greek (from the fallback: Geist has none) and Cyrillic, in every weight.
    for name in ["sans", "sans-600", "mono"] {
        let font = FontId::new(13.0, FontFamily::Name(name.into()));
        for sample in ["Hello", "Ελληνικά", "Кириллица"] {
            assert!(
                h.ctx.fonts_mut(|f| f.has_glyphs(&font, sample)),
                "{name}: {sample}"
            );
        }
    }
}

#[test]
fn text_styles_follow_the_type_scale() {
    let mut h = gui(synced_state());
    h.run();
    let style = h.ctx.global_style();
    let scale = TypeScale::DEFAULT;
    assert_eq!(style.text_styles[&TextStyle::Body].size, scale.body_md.size);
    assert_eq!(
        style.text_styles[&TextStyle::Button].family,
        FontFamily::Name("sans-500".into())
    );
    assert_eq!(
        style.text_styles[&TextStyle::Heading].family,
        FontFamily::Name("sans-600".into())
    );
    assert_eq!(
        style.text_styles[&TextStyle::Monospace].family,
        FontFamily::Name("mono-500".into())
    );
}

#[test]
fn unread_rows_are_semibold() {
    let scale = TypeScale::DEFAULT;
    // The text helper: a token at 600, as an unread row's sender.
    assert_eq!(
        theme::font_at(&scale.body_md, 600).family,
        FontFamily::Name("sans-600".into())
    );
    assert_eq!(theme::semibold(), FontFamily::Name("sans-600".into()));
    assert_eq!(
        theme::font(&scale.label_sm).family,
        FontFamily::Name("mono-500".into())
    );
}
