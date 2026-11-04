//! Embeds `packaging/windows/dail.manifest` into `dail.exe` (MSVC linker only), so that a
//! desktop launch shows just the GUI, without a console window, and links
//! `packaging/windows/dail.res` (the dail icon, from `cargo xtask icons`, MISC-39): the MSVC linker
//! takes a `.res` file as an input, so no resource compiler or C toolchain is needed.

use std::path::Path;

fn main() {
    let manifest =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging/windows/dail.manifest");
    let icon = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging/windows/dail.res");
    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rerun-if-changed={}", icon.display());
    let windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if windows && msvc {
        println!("cargo:rustc-link-arg-bin=dail=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bin=dail=/MANIFESTINPUT:{}",
            manifest.display()
        );
        println!("cargo:rustc-link-arg-bin=dail={}", icon.display());
    }
}
