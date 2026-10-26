# Bundled fonts

| File | Version | Source | Licence |
|---|---|---|---|
| `Geist[wght].ttf` | 1.800 | <https://github.com/google/fonts/tree/main/ofl/geist> (upstream <https://github.com/vercel/geist-font>) | SIL OFL 1.1, `OFL-Geist.txt` |
| `JetBrainsMono[wght].ttf` | 2.211 | <https://github.com/google/fonts/tree/main/ofl/jetbrainsmono> (upstream <https://github.com/JetBrains/JetBrainsMono>) | SIL OFL 1.1, `OFL-JetBrainsMono.txt` |

Taken unmodified on 2026-10-21. Both are variable fonts with one axis, `wght`; dail instances them at 400, 500 and
600 (`app_theme::fonts::WEIGHTS`). The licence texts ship next to the binary in every package
(`usr/share/doc/dail/` in the `.deb`).

## Icons

`MaterialSymbolsOutlined-dail.ttf` is a subset of Material Symbols Outlined (version 2.975, Apache-2.0,
`LICENSE-MaterialSymbols.txt`), from
<https://github.com/google/material-design-icons/tree/master/variablefont> at commit `49d4db35`, taken 2026-10-21.
It keeps the `FILL` (0–1) and `wght` (100–700) axes; `opsz` is fixed at 20 and `GRAD` at 0. The icons are the
variants of `app_theme::Icon` (`src/icon.rs`), with their code points from the upstream `.codepoints` file. Made with
fontTools 4.67:

```sh
fonttools varLib.instancer 'MaterialSymbolsOutlined[FILL,GRAD,opsz,wght].ttf' opsz=20 GRAD=0 -o inst.ttf
pyftsubset inst.ttf --unicodes="U+e159,U+f097,U+e156,U+e97f,U+e163,U+e66d,U+e149,U+e92e,U+f052,U+f09a,U+e2c7,U+e8b8,U+ef7a,U+f7e5,U+e627,U+e15e,U+e15f,U+f57a,U+f18c,U+f18a,U+e9a1,U+e5cd,U+e5ce,U+e5cf,U+e5d7,U+e5d6,U+e5d3,U+e226,U+f090,U+e89e,U+e873,U+e3f4,U+e415,U+e9de,U+e835,U+f083,U+f8b6,U+efd6,U+e166,U+e312,U+eb37,U+f0d3,U+e145,U+e8f4,U+e8f5,U+e250,U+e14d,U+f013,U+e2c1,U+e518,U+e51c,U+e5cc,U+e5cb,U+e668,U+e5d2" \
  --layout-features='' --no-hinting --desubroutinize --output-file=MaterialSymbolsOutlined-dail.ttf
```

`menu` (U+e5d2) was added on 2026-10-26 (MISC-2) with the same source, commands and fontTools version.

To add an icon: append its name and code point to `Icon`, add the code point to `--unicodes` and run both commands
again.
