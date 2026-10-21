//! The icon subset: unique names and code points, in the private use area, with a licence.

use std::collections::HashSet;

use app_theme::Icon;
use app_theme::icon::{ICON_FONT, ICON_LICENSE};

#[test]
fn icons_are_unique_and_private_use() {
    let chars: HashSet<char> = Icon::ALL.iter().map(|i| i.char()).collect();
    let names: HashSet<&str> = Icon::ALL.iter().map(|i| i.name()).collect();
    assert_eq!(chars.len(), Icon::ALL.len());
    assert_eq!(names.len(), Icon::ALL.len());
    for icon in Icon::ALL {
        assert!(('\u{e000}'..='\u{f8ff}').contains(&icon.char()), "{icon:?}");
    }
    assert_eq!(&ICON_FONT[..4], &[0, 1, 0, 0]);
    assert!(ICON_LICENSE.contains("Apache License"));
}
