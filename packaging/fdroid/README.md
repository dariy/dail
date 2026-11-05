# F-Droid (STORE-26)

`net.darii.dail.yml` is the recipe for fdroiddata. F-Droid builds the `live` flavour from source once per ABI and signs
it with its own key; reproducible builds are not part of the launch (ROADMAP Wave 11 decisions).

## Before the inclusion request

1. Tag the release the recipe names (`v0.9.0` above; update `versionName`, `versionCode`, `commit` and
   `CurrentVersion*` to it). The two version codes are the ABI ones in
   `fastlane/metadata/android/en-US/changelogs/`.
2. Test it locally with fdroidserver, from a checkout of fdroiddata with this file as `metadata/net.darii.dail.yml`:

   ```sh
   fdroid readmeta
   fdroid lint net.darii.dail
   fdroid fetchsrclibs net.darii.dail:<versionCode> --verbose
   fdroid build --verbose --test --on-server --no-tarball net.darii.dail:<versionCode>
   ```

   Run these as fdroiddata's CI does: in the `registry.gitlab.com/fdroid/fdroidserver:buildserver` image (Debian 13,
   which is why the recipe takes JDK 17 from bookworm), with a fdroidserver checkout on `PATH`, its `examples/` on
   `PYTHONPATH` (for `fetchsrclibs`) and `ANDROID_HOME` set to `/opt/android-sdk`. For example, with podman, from the
   fdroiddata checkout:

   ```sh
   podman run --rm -v "$PWD":/repo:Z -v /path/to/fdroidserver:/fdroidserver:Z -w /repo \
     -e ANDROID_HOME=/opt/android-sdk registry.gitlab.com/fdroid/fdroidserver:buildserver \
     bash -c 'export PATH=/fdroidserver:$PATH PYTHONPATH=/fdroidserver:/fdroidserver/examples
              fdroid fetchsrclibs net.darii.dail:<versionCode> &&
              fdroid build --verbose --test --on-server --no-tarball net.darii.dail:<versionCode>'
   ```

   The build needs network for crates.io (F-Droid allows it for Rust) and NDK r28c, which the `ndk:` field fetches.
3. Open the inclusion request (fdroiddata merge request, or the "Request For Packaging" form). F-Droid asks for three
   months of active development (dail reaches it on 2027-01-16) unless the app "has proven to have some users"; the
   closed test (STORE-24) can show that.
4. After the first F-Droid publish, add F-Droid's signing key to `net.darii.dail` for Android developer verification
   (STORE-27).

## What is not done here

The local `fdroid build` has not been run in the session that wrote this recipe: it needs fdroidserver's build
environment. The inclusion request is the user's to open.
