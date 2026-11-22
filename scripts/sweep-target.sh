#!/usr/bin/env bash
# Frees disk space in target/ without a full `cargo clean`.
#
# Every build unit (a crate built with one set of features/flags/target) leaves files named `<name>-<hash>` in
# <profile>/.fingerprint, deps, build, examples and incremental. Each feature or flag change makes a new hash and the
# old files stay. Cargo reads `.fingerprint/<name>-<hash>/*` each time the unit is part of a build, even if it is
# fresh, so the access time of those files shows when the unit was last used. This script removes all files of units
# not used for <days> days. After that, cargo rebuilds a removed unit only if a build needs it again.
#
# Usage: scripts/sweep-target.sh [--days N] [--dry-run] [--cross] [--all]
#   --days N   remove units not used for N days (default 7, at least 1)
#   --dry-run  print what would be removed and how much, change nothing
#   --cross    also remove the whole cross-target dirs (target/<triple>/: Android, iOS); `cargo xtask android|ios`
#              rebuilds them
#   --all      `cargo clean`: remove all of target/
# The time check needs atime (`relatime`, the Linux default, updates it once a day). On a `noatime` mount the script
# stops; use --all there.
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
target="${CARGO_TARGET_DIR:-$root/target}"
[ -d "$target" ] || { echo "nothing to do: $target does not exist"; exit 0; }

size() { du -sh "$target" 2>/dev/null | cut -f1; }
before=$(size)

run() { if [ $dry -eq 1 ]; then echo "would remove: $*"; else rm -rf -- "$@"; fi; }

# Cargo holds an exclusive lock on <profile>/.cargo-lock while it builds. Do not remove files under a running build.
locked() { [ -e "$1/.cargo-lock" ] && ! flock -n "$1/.cargo-lock" true 2>/dev/null; }

if [ $all -eq 1 ]; then
    if [ $dry -eq 1 ]; then echo "would run: cargo clean ($before in $target)"; exit 0; fi
    (cd "$root" && cargo clean)
    echo "target/: $before -> $(size)"
    exit 0
fi

mount_opts=$(findmnt -no OPTIONS --target "$target" 2>/dev/null || true)
case ",$mount_opts," in
    *,noatime,*) echo "error: $target is on a noatime mount; access times are not kept. Use --all." >&2; exit 1 ;;
esac

if [ $cross -eq 1 ]; then
    for dir in "$target"/*/; do
        dir=${dir%/}
        # A triple dir holds profile dirs (debug/, release/) and nothing named like a profile itself.
        [ -d "$dir/debug" ] || [ -d "$dir/release" ] || continue
        [ -d "$dir/.fingerprint" ] && continue
        if locked "$dir/debug" || locked "$dir/release"; then echo "skip $dir: cargo is building there"; continue; fi
        echo "cross target $(basename "$dir"): $(du -sh "$dir" | cut -f1)"
        run "$dir"
    done
fi

total_units=0
total_stale=0
# Profile dirs: target/<profile> and target/<triple>/<profile>, each with a .fingerprint dir.
while IFS= read -r fp; do
    profile=$(dirname "$fp")
    [ -d "$profile" ] || continue                                    # removed above by --cross
    [ $dry -eq 1 ] && [ $cross -eq 1 ] && [ "$(dirname "$profile")" != "$target" ] && continue  # would be
    if locked "$profile"; then echo "skip ${profile#"$target"/}: cargo is building there"; continue; fi

    # Units whose fingerprint files were read in the last <days> days.
    live=$(find "$fp" -mindepth 2 -maxdepth 2 -type f -amin "-$((days * 1440))" -printf '%h\n' | sort -u |
        sed -n 's/.*-\([0-9a-f]\{16\}\)$/\1/p')
    all_units=$(find "$fp" -mindepth 1 -maxdepth 1 -type d -printf '%f\n' | sed -n 's/.*-\([0-9a-f]\{16\}\)$/\1/p')
    stale=$(comm -23 <(sort -u <<<"$all_units") <(sort -u <<<"$live"))
    n_all=$(grep -c . <<<"$all_units" || true)
    n_stale=$(grep -c . <<<"$stale" || true)
    total_units=$((total_units + n_all))
    total_stale=$((total_stale + n_stale))
    echo "${profile#"$target"/}: $n_stale of $n_all units unused for $days+ days"
    [ "$n_stale" -gt 0 ] || continue

    # Every entry whose name carries a stale hash: `name-<hash>`, `name-<hash>.ext`, `libname-<hash>.rlib`, ...
    victims=$(mktemp)
    for sub in .fingerprint deps build examples incremental; do
        [ -d "$profile/$sub" ] || continue
        find "$profile/$sub" -mindepth 1 -maxdepth 1 -printf '%p\n'
    done | awk -v stale="$stale" '
        BEGIN { n = split(stale, h, "\n"); for (i = 1; i <= n; i++) if (h[i] != "") keep[h[i]] = 1 }
        {
            name = $0; sub(/.*\//, "", name)
            if (match(name, /-[0-9a-f]{16}(\.|$)/)) {
                hash = substr(name, RSTART + 1, 16)
                if (hash in keep) print
            }
        }' >"$victims"
    # Left over by an interrupted rustc.
    find "$profile/deps" -mindepth 1 -maxdepth 1 -type d -name 'rustc*' -mtime +0 -printf '%p\n' 2>/dev/null >>"$victims" || true

    if [ $dry -eq 1 ]; then
        echo "  would free $(tr '\n' '\0' <"$victims" | du -csh --files0-from=- 2>/dev/null | tail -1 | cut -f1)"
    else
        tr '\n' '\0' <"$victims" | xargs -0 -r rm -rf --
    fi
    rm -f "$victims"
done < <(find "$target" -mindepth 2 -maxdepth 3 -type d -name .fingerprint | sort)

if [ $dry -eq 1 ]; then
    echo "dry run: $total_stale of $total_units units would be removed; target/ is $before"
else
    echo "removed $total_stale of $total_units units; target/: $before -> $(size)"
fi
