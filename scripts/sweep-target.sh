#!/usr/bin/env bash
# Frees disk space in target/ without a full `cargo clean`, with cargo-sweep (`cargo install cargo-sweep --locked`).
#
# Each change of features, flags, build mode, target or toolchain makes cargo build another copy of the crates it
# affects, and the old copies stay in target/. This script runs, in order:
#   1. `cargo sweep --time <days>`: removes the copies not used for <days> days.
#   2. `cargo sweep --installed`: removes the copies made by toolchains rustup no longer has (after a toolchain update).
#   3. Project additions: the temp dirs an interrupted rustc leaves in deps/, and with --cross the Android and iOS
#      target dirs.
# Cargo rebuilds a removed copy only if a build needs it again. The global cache (~/.cargo/registry, git) is cleaned
# by cargo itself (automatic gc, stable since 1.88).
#
# Usage: scripts/sweep-target.sh [--days N] [--dry-run] [--cross] [--all]
#   --days N   remove copies not used for N days (default 7)
#   --dry-run  print what would be removed and how much, change nothing
#   --cross    also remove the whole cross-target dirs (target/<triple>/: Android, iOS); `cargo xtask android|ios`
#              rebuilds them. (`cargo clean --target <triple>` would remove all of target/.)
#   --all      `cargo clean`: remove all of target/
set -euo pipefail

days=7
dry=0
cross=0
all=0
while [ $# -gt 0 ]; do
    case $1 in
        --days) days=${2:?--days needs a number}; shift 2 ;;
        --dry-run) dry=1; shift ;;
        --cross) cross=1; shift ;;
        --all) all=1; shift ;;
        -h|--help) sed -n '2,/^set -euo/{/^set/d;s/^# \{0,1\}//;p}' "$0"; exit 0 ;;
        *) echo "error: unknown argument: $1 (see --help)" >&2; exit 2 ;;
    esac
done
case $days in ''|*[!0-9]*|0) echo "error: --days needs a whole number of at least 1, not '$days'" >&2; exit 2 ;; esac

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
target="${CARGO_TARGET_DIR:-$root/target}"
[ -d "$target" ] || { echo "nothing to do: $target does not exist"; exit 0; }

size() { du -sh "$target" 2>/dev/null | cut -f1; }
before=$(size)

if [ $all -eq 1 ]; then
    if [ $dry -eq 1 ]; then cargo clean --dry-run; else cargo clean; echo "target/: $before -> $(size)"; fi
    exit 0
fi

if ! cargo sweep --version >/dev/null 2>&1; then
    echo "error: cargo-sweep is missing; install it with: cargo install cargo-sweep --locked" >&2
    exit 1
fi

# Cargo holds an exclusive lock on <profile>/.cargo-lock while it builds; cargo-sweep does not check it.
while IFS= read -r lock; do
    if ! flock -n "$lock" true 2>/dev/null; then
        echo "error: cargo is building in $(dirname "$lock"); run this again when it has finished" >&2
        exit 1
    fi
done < <(find "$target" -mindepth 2 -maxdepth 3 -name .cargo-lock)

dry_flag=()
[ $dry -eq 1 ] && dry_flag=(--dry-run)
run() { if [ $dry -eq 1 ]; then echo "would remove: $*"; else rm -rf -- "$@"; fi; }

# 3a, first so that cargo-sweep does not scan what goes anyway.
if [ $cross -eq 1 ]; then
    for dir in "$target"/*/; do
        dir=${dir%/}
        # A triple dir holds profile dirs (debug/, release/); a profile dir holds .fingerprint.
        [ -d "$dir/debug" ] || [ -d "$dir/release" ] || continue
        [ -d "$dir/.fingerprint" ] && continue
        echo "cross target $(basename "$dir"): $(du -sh "$dir" | cut -f1)"
        run "$dir"
    done
fi

# 1 and 2. cargo-sweep takes one mode per call.
cargo sweep "${dry_flag[@]}" --time "$days" "$root"
cargo sweep "${dry_flag[@]}" --installed "$root"

# 3b. rustcXXXXXX temp dirs older than a day.
while IFS= read -r tmp; do run "$tmp"; done < <(
    find "$target" -mindepth 3 -maxdepth 4 -path '*/deps/rustc*' -type d -mtime +0 2>/dev/null)

if [ $dry -eq 1 ]; then echo "dry run: target/ is $before"; else echo "target/: $before -> $(size)"; fi
