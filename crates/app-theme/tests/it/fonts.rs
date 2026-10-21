//! The bundled fonts: present, with their licences, and a family per face and weight.

use app_theme::fonts::{self, WEIGHTS};
use app_theme::{Face, TypeScale};

#[test]
fn fonts_are_bundled_with_their_licences() {
    for font in fonts::fonts() {
        // A TrueType file starts with the version 1.0 tag.
        assert_eq!(&font.bytes[..4], &[0, 1, 0, 0], "{}", font.name);
        assert!(
            font.license.contains("SIL Open Font License"),
            "{}",
            font.name
        );
        assert_eq!(font.axes, ["wght"]);
        assert_eq!(fonts::file(font.face), font);
    }
}

#[test]
fn every_face_and_weight_has_its_own_family() {
    let mut names: Vec<&str> = [Face::Sans, Face::Mono]
        .into_iter()
        .flat_map(|face| WEIGHTS.map(|w| fonts::family(face, w)))
        .collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 6);
    assert_eq!(fonts::family(Face::Sans, 400), "sans");
    assert_eq!(fonts::family(Face::Mono, 600), "mono-600");
    assert_eq!(fonts::nearest_weight(700), 600);
    assert_eq!(fonts::nearest_weight(100), 400);
    // Every token of the scale uses a bundled weight.
    for token in TypeScale::DEFAULT.all() {
        assert!(WEIGHTS.contains(&token.weight), "{token:?}");
    }
}
