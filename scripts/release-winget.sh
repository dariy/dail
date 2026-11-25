#!/usr/bin/env bash
# Puts a released MSI on winget (DIST-3): opens a pull request on microsoft/winget-pkgs for package `dariy.dail`
# with komac (https://github.com/russellbanks/Komac), downloaded at a pinned version and checked by sha256.
#
#   scripts/release-winget.sh update [<x.y.z>]   add a version; non-interactive (the `winget` job of release.yml)
#   scripts/release-winget.sh new [<x.y.z>]      the first submission, once, by hand (komac asks the rest in a TTY)
#
# The version defaults to the workspace's; its GitHub Release must be published, since komac downloads the MSI from it
# to read the hash and ProductCode. Needs `GITHUB_TOKEN`: a classic PAT with `public_repo` from the account that owns
# the winget-pkgs fork (komac creates the fork if it is missing). `DRY_RUN=1` prints the manifests and submits
# nothing. Linux x86_64 only, like the release runners.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

komac_version=2.17.2
komac_sha256=b03b020eec98acfb0b53e284a6e21815b884be33de6785b97e7a120f0e748966
identifier=dariy.dail

mode="${1:-}"
case "$mode" in
    new|update) ;;
    *) echo "usage: $0 new|update [<x.y.z>]" >&2; exit 2 ;;
esac
# `cargo pkgid -p app-cli` prints `path+file:///…/crates/app-cli#app-cli@0.1.0`.
version="${2:-$(cargo pkgid -p app-cli | sed 's/^.*[#@]//')}"
if [ -z "${GITHUB_TOKEN:-}" ]; then
    echo "error: GITHUB_TOKEN is not set (a classic PAT with public_repo)" >&2
    exit 1
fi

tools="$(mktemp -d)"
trap 'rm -rf "$tools"' EXIT
archive="komac-$komac_version-x86_64-unknown-linux-gnu.tar.gz"
curl -fsSL -o "$tools/$archive" "https://github.com/russellbanks/Komac/releases/download/v$komac_version/$archive"
echo "$komac_sha256  $tools/$archive" | sha256sum -c --quiet -
tar -xzf "$tools/$archive" -C "$tools" komac

repo=https://github.com/dariy/dail
msi="$repo/releases/download/v$version/dail-$version-x86_64.msi"
notes="$repo/releases/tag/v$version"
export KOMAC_CREATED_WITH="dail release-winget.sh"
export KOMAC_CREATED_WITH_URL="$repo/blob/main/scripts/release-winget.sh"

submit=(--submit)
if [ -n "${DRY_RUN:-}" ]; then
    submit=(--dry-run)
fi

if [ "$mode" = update ]; then
    # The locale fields (description, tags, licence) carry over from the version already in winget-pkgs.
    "$tools/komac" update "$identifier" --version "$version" --urls "$msi" --release-notes-url "$notes" "${submit[@]}"
else
    # The text of the package page; DIST-7 keeps it in step with the other stores.
    "$tools/komac" new "$identifier" --version "$version" --urls "$msi" \
        --package-locale en-US \
        --publisher "dail developers" \
        --publisher-url https://github.com/dariy \
        --publisher-support-url "$repo/issues" \
        --package-name dail \
        --package-url "$repo" \
        --moniker dail \
        --author "Alexei Darii" \
        --license "MIT OR Apache-2.0" \
        --license-url "$repo/blob/main/LICENSE-MIT" \
        --copyright "Copyright (c) 2026 Alexei Darii" \
        --short-description "A keyboard-driven email client that agents can drive: GUI, TUI, CLI and MCP server." \
        --description "dail is a local-first IMAP and SMTP mail client. Every action is a command that the desktop GUI, the terminal UI, the command line and the built-in MCP server can run, so an AI agent can read, sort and draft mail while sending stays behind your approval. Passwords stay in the Windows Credential Manager." \
        --release-notes-url "$notes" \
        "${submit[@]}"
fi
