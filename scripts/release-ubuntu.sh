#!/usr/bin/env bash
# Builds the release `dail` binary and packages it as dist/dail-<version>-x86_64-unknown-linux-gnu.tar.gz and
# dist/dail_<version>-1_amd64.deb (+ .sha256 for each). Needs cargo-deb (`cargo install cargo-deb --locked`).
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

cd "$dist"
for file in "$name.tar.gz" dail_*.deb; do
    sha256sum "$file" >"$file.sha256"
    echo "built $dist/$file"
done
