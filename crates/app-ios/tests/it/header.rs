//! `include/dail.h` declares exactly what `src/ffi.rs` exports, with the same struct fields.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use std::collections::BTreeSet;

fn read(path: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(root.join(path)).unwrap()
}

/// The identifier right before the first `(` of each line that declares a `dail_` function.
fn header_functions(header: &str) -> BTreeSet<String> {
    header
        .lines()
        .filter(|line| !line.trim_start().starts_with("/*") && line.contains("dail_"))
        .filter_map(|line| {
            let name = line.split('(').next()?.split([' ', '*']).next_back()?;
            name.starts_with("dail_").then(|| name.to_owned())
        })
        .collect()
}

/// The `extern "C" fn` names right after `#[unsafe(no_mangle)]`.
fn rust_exports(source: &str) -> BTreeSet<String> {
    let mut lines = source.lines().map(str::trim);
    let mut names = BTreeSet::new();
    while let Some(line) = lines.next() {
        if line == "#[unsafe(no_mangle)]" {
            let decl = lines.next().unwrap();
            let name = decl.split("fn ").nth(1).unwrap().split('(').next().unwrap();
            assert!(decl.contains("extern \"C\" fn"), "{decl}");
            names.insert(name.to_owned());
        }
    }
    names
}

/// The text between the `{` after `opener` and the next `}`.
fn body<'a>(source: &'a str, opener: &str) -> &'a str {
    let after = source.split(opener).nth(1).unwrap();
    &after[after.find('{').unwrap() + 1..after.find('}').unwrap()]
}

/// The field names of a C struct: one declaration per line, `type name;` or `ret (*name)(…);`.
fn c_fields(header: &str, name: &str) -> Vec<String> {
    body(header, &format!("typedef struct {name}"))
        .lines()
        .map(str::trim)
        .filter(|line| line.ends_with(';'))
        .map(|line| match line.split_once("(*") {
            Some((_, rest)) => rest.split(')').next().unwrap().to_owned(),
            None => line
                .trim_end_matches(';')
                .rsplit([' ', '*'])
                .next()
                .unwrap()
                .to_owned(),
        })
        .collect()
}

/// The field names of a Rust struct: one `pub name: Type,` per line.
fn rust_fields(source: &str, name: &str) -> Vec<String> {
    body(source, &format!("pub struct {name}"))
        .lines()
        .filter_map(|line| line.trim().strip_prefix("pub "))
        .map(|line| line.split(':').next().unwrap().to_owned())
        .collect()
}

#[test]
fn the_header_declares_every_export() {
    let header = header_functions(&read("include/dail.h"));
    let exports = rust_exports(&read("src/ffi.rs"));
    assert_eq!(header, exports);
    assert_eq!(exports.len(), 8, "six calls and two frees");
}

#[test]
fn the_structs_have_the_same_fields() {
    let header = read("include/dail.h");
    let source = read("src/ffi.rs");
    for name in ["DailListener", "DailBytes"] {
        let c = c_fields(&header, name);
        let rust = rust_fields(&source, name);
        assert_eq!(c, rust, "{name}");
        assert!(!c.is_empty(), "{name}");
    }
}
