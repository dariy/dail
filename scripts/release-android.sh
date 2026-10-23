#!/usr/bin/env bash
# Builds the signed release APKs, one per ABI, as dist/dail-<version>-<abi>.apk (+ .sha256 for each), and
# prints their sizes. Needs what `cargo xtask android apk` needs (cargo-ndk, NDK r28+, JDK 17, the SDK) and the
# signing key in DAIL_SIGNING_STORE, DAIL_SIGNING_STORE_PASSWORD, DAIL_SIGNING_KEY_ALIAS and
# DAIL_SIGNING_KEY_PASSWORD (android/app/build.gradle.kts). Run from anywhere: `scripts/release-android.sh`.
# The release workflow (.github/workflows/release.yml) runs it too.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

for var in DAIL_SIGNING_STORE DAIL_SIGNING_STORE_PASSWORD DAIL_SIGNING_KEY_ALIAS DAIL_SIGNING_KEY_PASSWORD; do
    if [ -z "${!var:-}" ]; then
        echo "error: $var is not set; a release APK must be signed" >&2
        exit 1
    fi
done

sdk="${ANDROID_HOME:-$(sed -n 's/^sdk\.dir=//p' android/local.properties 2>/dev/null || true)}"
apksigner="$(ls -d "$sdk"/build-tools/*/apksigner 2>/dev/null | sort -V | tail -1 || true)"
if [ -z "$apksigner" ]; then
    echo "error: no apksigner under $sdk/build-tools (set ANDROID_HOME)" >&2
    exit 1
fi

version="$(cargo pkgid -p app-android | sed 's/^.*[#@]//')"

# The licence lists must be current, then the release APKs are built and checked like the gate's.
cargo run -q -p xtask -- android licences --check
rm -rf android/app/build/outputs/apk
(cd android && ./gradlew assembleLiveRelease)
cargo run -q -p xtask -- android apk-check

dist="$root/dist"
mkdir -p "$dist"
rm -f "$dist"/dail-*.apk "$dist"/dail-*.apk.sha256
for apk in android/app/build/outputs/apk/live/release/*.apk; do
    # app-live-arm64-v8a-release.apk → arm64-v8a
    abi="$(basename "$apk" .apk | sed 's/^app-live-//; s/-release$//')"
    case "$abi" in *unsigned*)
        echo "error: $apk is unsigned" >&2
        exit 1
        ;;
    esac
    name="dail-$version-$abi.apk"
    cp "$apk" "$dist/$name"
    "$apksigner" verify --min-sdk-version 29 "$dist/$name"
    (cd "$dist" && sha256sum "$name" >"$name.sha256")
    echo "built $dist/$name ($(du -k "$dist/$name" | cut -f1) KiB)"
done
