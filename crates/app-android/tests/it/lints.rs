//! The crate's `[lints]` table is the workspace's but for `unsafe_code` (ADR 0009).

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use toml::Table;

fn read(path: &str) -> Table {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(root.join(path))
        .unwrap()
        .parse()
        .unwrap()
}

#[test]
fn the_lints_are_the_workspaces_but_unsafe_code_is_deny() {
    let workspace = read("../../Cargo.toml")["workspace"]["lints"].clone();
    let mut ours = read("Cargo.toml")["lints"].clone();
    assert_eq!(ours["rust"]["unsafe_code"].as_str(), Some("deny"));
    assert_eq!(workspace["rust"]["unsafe_code"].as_str(), Some("forbid"));
    ours["rust"]
        .as_table_mut()
        .unwrap()
        .insert("unsafe_code".to_owned(), "forbid".into());
    assert_eq!(ours, workspace);
}

#[test]
fn only_jni_rs_allows_unsafe_code() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for file in std::fs::read_dir(&src).unwrap() {
        let path = file.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        let allows = text.contains("allow(unsafe_code)");
        let is_jni = path.file_name().unwrap() == "jni.rs";
        assert_eq!(allows, is_jni, "{}", path.display());
    }
}
