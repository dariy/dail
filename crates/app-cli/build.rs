//! Embeds `packaging/windows/dail.manifest` into `dail.exe` (MSVC linker only), so that a
//! desktop launch shows just the GUI, without a console window.

use std::path::Path;

fn main() {
    let manifest =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging/windows/dail.manifest");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if windows && msvc {
        println!("cargo:rustc-link-arg-bin=dail=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bin=dail=/MANIFESTINPUT:{}",
            manifest.display()
        );
    }
}
