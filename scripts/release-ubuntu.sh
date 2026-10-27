#!/usr/bin/env bash
# Builds the release `dail` binary and packages it as dist/dail-<version>-x86_64-unknown-linux-gnu.tar.gz and
# dist/dail_<version>-1_amd64.deb, dist/dail-<version>-x86_64.AppImage and the bare binary
# dist/dail-<version>-x86_64-unknown-linux-gnu (+ .sha256 for each). Needs cargo-deb
# (`cargo install cargo-deb --locked`) and curl; appimagetool is downloaded at a pinned version and checked by sha256.
# Run from anywhere: `scripts/release-ubuntu.sh`. The release workflow (.github/workflows/release.yml) runs it too.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

if ! cargo deb --version >/dev/null 2>&1; then
    echo "error: cargo-deb is missing; install it with: cargo install cargo-deb --locked" >&2
    exit 1
fi

# `cargo pkgid -p app-cli` prints `path+file:///…/crates/app-cli#app-cli@0.1.0`.
version="$(cargo pkgid -p app-cli | sed 's/^.*[#@]//')"
target="$(rustc -vV | sed -n 's/^host: //p')"
name="dail-$version-$target"

# [profile.release] does thin LTO and strips.
cargo build --release --locked -p app-cli

dist="$root/dist"
stage="$dist/$name"
rm -rf "$stage"
mkdir -p "$stage"
cp target/release/dail packaging/linux/dail.desktop packaging/linux/dail.svg "$stage/"
tar -C "$dist" -czf "$dist/$name.tar.gz" "$name"
rm -rf "$stage"

# [package.metadata.deb] in crates/app-cli/Cargo.toml: the binary, desktop entry and icon.
rm -f "$dist"/dail_*.deb
cargo deb -p app-cli --no-build --output "$dist"

# AppImage (Wave 7 R1): an AppDir with the binary, desktop entry and icon, packed by appimagetool. The tool runs
# with --appimage-extract-and-run so the runner needs no FUSE.
appimagetool_url="https://github.com/AppImage/appimagetool/releases/download/1.9.0/appimagetool-x86_64.AppImage"
appimagetool_sha256="46fdd785094c7f6e545b61afcfb0f3d98d8eab243f644b4b17698c01d06083d1"
tools="$root/target/release-tools"
mkdir -p "$tools"
appimagetool="$tools/appimagetool-1.9.0"
if ! echo "$appimagetool_sha256  $appimagetool" | sha256sum -c --status 2>/dev/null; then
    curl -sSLf -o "$appimagetool" "$appimagetool_url"
    echo "$appimagetool_sha256  $appimagetool" | sha256sum -c --status || {
        echo "error: appimagetool does not match its pinned sha256" >&2
        exit 1
    }
fi
chmod +x "$appimagetool"
appdir="$dist/dail.AppDir"
rm -rf "$appdir"
mkdir -p "$appdir/usr/bin"
cp target/release/dail "$appdir/usr/bin/dail"
cp packaging/linux/dail.desktop packaging/linux/dail.svg "$appdir/"
cp packaging/linux/AppRun "$appdir/AppRun"
chmod +x "$appdir/AppRun"
appimage="dail-$version-x86_64.AppImage"
rm -f "$dist/$appimage"
ARCH=x86_64 "$appimagetool" --appimage-extract-and-run --no-appstream "$appdir" "$dist/$appimage" >/dev/null
rm -rf "$appdir"

# The bare binary: what a plain-binary install replaces itself with (ADR 0011, kind `exe`).
cp target/release/dail "$dist/$name"

cd "$dist"
for file in "$name.tar.gz" dail_*.deb "$appimage" "$name"; do
    sha256sum "$file" >"$file.sha256"
    echo "built $dist/$file"
done
