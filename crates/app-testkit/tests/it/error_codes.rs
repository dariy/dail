//! Every error the user can read carries a unique, stable code (MISC-35, ADR 0013).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Parser details and test fixtures: never shown on their own.
const UNCODED: &[&str] = &["app-core/src/search.rs", "app-testkit/src/fixture.rs"];

fn rust_files(dir: &Path, found: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            rust_files(&path, found)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
    Ok(())
}

#[test]
fn error_messages_have_unique_codes() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut files = Vec::new();
    rust_files(crates, &mut files).unwrap();
    let mut seen = BTreeMap::new();
    for file in files
        .iter()
        .filter(|file| file.components().any(|c| c.as_os_str() == "src"))
    {
        let name = file
            .strip_prefix(crates)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if !name.contains("/src/") || UNCODED.contains(&name.as_str()) {
            continue;
        }
        let text = std::fs::read_to_string(file).unwrap();
        for (at, _) in text.match_indices("#[error(") {
            let rest = text[at + "#[error(".len()..].trim_start();
            if rest.starts_with("transparent") {
                continue;
            }
            let code = rest
                .strip_prefix('"')
                .and_then(|rest| rest.get(..6))
                .unwrap_or("");
            assert!(
                code.starts_with("ERR") && code[3..].bytes().all(|b| b.is_ascii_digit()),
                "{name}: error message without a code: {rest:.40}"
            );
            let earlier = seen.insert(code.to_owned(), name.clone());
            assert!(
                earlier.is_none(),
                "{code} is used twice: {earlier:?}, {name}"
            );
        }
    }
    assert!(seen.len() > 40, "the scan found only {} codes", seen.len());
}
