# 0012 — PDF pages by hayro in app-io

- Status: accepted
- Date: 2026-10-28
- Plan: `ROADMAP.md`, Wave 9, MISC-19 (`docs/requests/misc.md`)

## Context

MISC-19 opens JPG and PDF attachments inside the mobile apps, with zoom. Every PDF that reaches dail is
attacker-controlled: it comes from a stranger's mail. A PDF is a container of compressed streams, fonts, images in
five codecs, functions and a page description language, so its parser is a large attack surface. Pictures and HTML are
already parsed by memory-safe Rust only (`app_io::images`, `app_io::html`); PDF pages need a renderer too, and it
runs in dail's process, next to the account passwords' keyring handles.

Options considered:

| Option | Why not (or why) |
|---|---|
| Android `PdfRenderer` | PDFium: C++, in process, a long list of memory-safety CVEs. Android only. |
| iOS PDFKit / CoreGraphics | C/C++ in process; its JBIG2 decoder was the FORCEDENTRY zero-click exploit. iOS only. |
| Hand the file to another app | The PDF leaves dail. Only *Save* may do that, by the user's choice. |
| pdf.js in a WebView | A JavaScript engine for every PDF, a WebView in both apps, and no `cargo test`. |
| C bindings (MuPDF, Poppler, PDFium) | Banned: no C/C++ at build time (AGENTS.md §5). |
| pdfcraft's engine crates | Git-only, built on a patched hayro 0.7 whose `[patch]` entries Cargo does not pass on to dependents, and they render with hayro anyway. |
| **hayro** | Pure Rust, `#![forbid(unsafe_code)]` in its parser-facing crates, one renderer for both apps that `cargo test` covers. Its known bugs are crashes and hangs, not memory corruption. |

## Decision

**Engine.** `hayro` renders PDF pages in a new `app_io::pdf`, beside `images`. Both apps get pages through the
host's `blob` call, as RGBA, like pictures.

**Source.** Upstream `main` at commit `5a4ddbb861e97d7693400c4d8d97363caa2c884f` (2026-10-28, CI green), as a git
dependency, not crates.io 0.8.0. `main` carries 190 commits of hardening since `hayro-v0.8.0`, among them the JBIG2
limits (#1529 to #1671), the PostScript-function stack fix (#1656), mesh-shading bounds (#1645) and the object
nesting limit (#1556). `deny.toml` allows this one git source.

`hayro-postscript` at that commit reads `<<` and `>>` by calling itself once per delimiter, so a CMap with a long run
of them overflows the stack and aborts the app (`object.rs`, marked `TODO` upstream). A copy patched to skip the
delimiters in a loop lives in `vendor/hayro-postscript/` (pdfcraft's fix, with two regression tests) and replaces the
upstream crate through `[patch]`. `vendor/` is excluded from the workspace. When a crates.io release carries the
fixes, dail goes back to it and drops the copy (a future roadmap item).

**Features.** `default-features = false, features = ["embed-fonts", "embed-cmaps"]`: the standard 14 fonts and the
predefined CMaps are embedded, so no font is looked up on the device.

**Unsafe code.** Accepted, and listed here exactly. These crates forbid it: hayro, hayro-interpret, hayro-cmap,
hayro-postscript, hayro-jbig2, hayro-jpeg2000, hayro-ccitt, vello_cpu, vello_common. Unsafe that remains:
- hayro-syntax: one `transmute` that extends an xref borrow (`page.rs`), plus its `unsafe` feature, which
  hayro-interpret turns on: flate2 with zlib-rs, memchr, and zune-jpeg's x86 and NEON paths.
- fearless_simd (used by hayro and vello).
- pic-scale, with hayro's `neon`, `sse` and `avx` features.
- smallvec, bytemuck, as elsewhere in the tree.

hayro-jbig2's and hayro-jpeg2000's own `simd` features stay off.

**Zoom.** Pinch zooms the whole document. Deep zoom uses tiles: a page is rendered once at the fit width as the base
image, and the visible area at the zoom level is rendered in tiles of at most `TILE_MAX` px. hayro's public
`render_into(page, …, ctx, transform)` renders a tile by shifting the transform; hayro needs no patch for it. This
replaces the request's re-render of the whole page capped at `MAX_SIDE`.

**Limits in `app_io::pdf`.**
- A file of at most `MAX_BYTES` = 32 MiB and `MAX_PAGES` = 1000 pages.
- A base raster of 1 to `MAX_SIDE` (4096) px a side.
- A virtual page width for tiles of at most `MAX_ZOOM_WIDTH` = 16384 px; a tile of at most `TILE_MAX` = 1024 px a
  side, inside the page.
- Pages whose size is not finite or smaller than 1 pt are refused.
- Open and render run under `catch_unwind`; a panic is an error.
- The host runs one PDF job at a time, with a 10 s limit; an overrun answers `None`.
- Annotations are drawn as their printed appearance. No links, no forms, no JavaScript, no remote loads.

**Quality.** hayro does not yet draw blend modes in knockout groups or colour-key masks, and some CID fonts are
unsupported. A page that fails shows "This page could not be shown." with *Save*.

**Viewer.** Android adapts pdfcraft-android's viewer (`DocLayout`, `PageCanvas`, `PageImages`, part of
`ViewerState`; MIT OR Apache-2.0, © 2026 ArtCraft Team and the PdfCraft contributors), trimmed to viewing. The MIT
notice goes into each adapted file and into About. iOS ports the layout to Swift with the same notice.

**Licences.** About credits the data hayro embeds: the 14 Foxit Type 1 fonts from PDFium (BSD-3-Clause), the Adobe
CMaps (BSD-3-Clause) and the CGATS001 ICC profile (CC0). `cargo xtask android licences` writes them to
`crates/app-mobile/licences/pdf-assets.txt` and checks it for staleness.

## Consequences

- A failed allocation still aborts the process, and a decompression bomb inside hayro is not bounded yet (upstream
  PR #1663 is open). The preview is not restored on restart, so such a file cannot crash-loop the app.
- A render cannot be interrupted; the single permit and the timeout bound what a hostile file costs.
- Future hardening: render in an isolated Android process, as Chrome does with PDFium.
- The git dependency and the vendored crate must be followed by hand until a release lands.
- Speed, size and the new crates are measured when `app_io::pdf` lands (below).

### Measurements

To be filled in by the `app_io::pdf` commit: page and tile render time on the emulator, the size change of
`libdail.so` and the APK, and the new crates in `cargo tree -p app-android -e normal --target aarch64-linux-android`.
