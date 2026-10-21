use app_theme::{Rgba, Theme, audit, builtin, contrast, hex};

#[test]
fn every_builtin_passes_the_audit() {
    for theme in builtin::all() {
        assert_eq!(audit(&theme), vec![], "{}", theme.id);
    }
}

#[test]
fn audit_reports_a_failing_pair() {
    let mut theme = builtin::get("dail-light").unwrap();
    theme.content.text_muted = hex(0x8E9192); // the mockup's muted text: 3.17:1 on white
    let problems = audit(&theme);
    assert!(
        problems
            .iter()
            .any(|p| p.group == "content" && p.fg == "text_muted" && p.bg == "base")
    );
    assert!(problems.iter().all(|p| p.ratio < p.min));
}

#[test]
fn ids_are_unique_slugs() {
    let ids: Vec<_> = builtin::all().into_iter().map(|t| t.id).collect();
    for id in &ids {
        assert!(
            id.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
            "{id}"
        );
        assert!(!id.starts_with('-') && !id.ends_with('-'), "{id}");
    }
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len());
    assert_eq!(builtin::get("dail-dark").unwrap().id, "dail-dark");
    assert!(builtin::get("nope").is_none());
}

#[test]
fn themes_round_trip_through_json() {
    for theme in builtin::all() {
        let json = serde_json::to_string(&theme).unwrap();
        let back: Theme = serde_json::from_str(&json).unwrap();
        assert_eq!(back, theme);
    }
}

#[test]
fn scales_increase() {
    let m = builtin::all()[0].metrics;
    let space = [m.space.xs, m.space.sm, m.space.md, m.space.lg, m.space.xl];
    assert!(space.windows(2).all(|w| w[0] < w[1]), "{space:?}");
    let radius = [m.radius.sm, m.radius.md, m.radius.lg, m.radius.full];
    assert!(radius.windows(2).all(|w| w[0] < w[1]), "{radius:?}");
    assert!(m.rows.compact < m.rows.cozy && m.rows.cozy < m.rows.preview);
}

#[test]
fn chrome_groups() {
    let light = builtin::get("dail-light").unwrap();
    assert!(light.split_chrome());
    assert_eq!(light.chrome, builtin::DARK);
    assert!(!builtin::get("dail-dark").unwrap().split_chrome());
}

#[test]
fn contrast_math() {
    let (black, white) = (hex(0x000000), hex(0xFFFFFF));
    assert!((contrast(black, white) - 21.0).abs() < 1e-9);
    assert!((contrast(white, black) - 21.0).abs() < 1e-9);
    assert!((contrast(white, white) - 1.0).abs() < 1e-9);
    // Known value: #767676 on white is the classic 4.54:1.
    assert!((contrast(hex(0x767676), white) - 4.54).abs() < 0.01);
    // A translucent colour is blended over the background first.
    assert_eq!(Rgba { a: 0, ..black }.over(white), white);
    assert!((contrast(black.with_alpha(0), white) - 1.0).abs() < 1e-9);
}
