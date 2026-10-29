//
// Adapted from pdfcraft-android apps/pdfcraft-android/android/app/src/main/java/io/github/artcraftandroid/pdfcraft/ui/viewer/DocLayout.kt@796fa92874ecc92247af95b216b0dad2eee04a08
// (ported to Swift and trimmed to one continuous column).
//
// Copyright (c) 2026 ArtCraft Team and the PdfCraft contributors
// MIT License: Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
// associated documentation files (the "Software"), to deal in the Software without restriction, including without
// limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the
// Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:
// The above copyright notice and this permission notice shall be included in all copies or substantial portions of
// the Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
// NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO
// EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN
// ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

import CoreGraphics

/// Where each page sits at zoom 1, in points, for a viewport `viewWidth` wide: one column with
/// `gap` between pages, scaled together so the widest fills the width. Pure geometry, tested in
/// `DocumentLayoutTests`.
struct DocumentLayout {
    let pages: [CGSize]
    let rects: [CGRect]
    let size: CGSize

    init(pages: [CGSize], viewWidth: CGFloat, gap: CGFloat) {
        self.pages = pages
        let widest = max(1, pages.map(\.width).max() ?? 1)
        let scale = max(1, viewWidth) / widest
        var y: CGFloat = 0
        var rects: [CGRect] = []
        for page in pages {
            let w = max(1, page.width) * scale
            let h = max(1, page.height) * scale
            rects.append(CGRect(x: (viewWidth - w) / 2, y: y, width: w, height: h))
            y += h + gap
        }
        self.rects = rects
        size = CGSize(width: viewWidth, height: pages.isEmpty ? 0 : y - gap)
    }

    /// The page nearest layout row `y`: the one under it, else the closest.
    func nearestPage(_ y: CGFloat) -> Int {
        guard !rects.isEmpty else { return 0 }
        var best = 0
        var distance = CGFloat.greatestFiniteMagnitude
        for (i, r) in rects.enumerated() {
            let d = y < r.minY ? r.minY - y : (y > r.maxY ? y - r.maxY : 0)
            if d < distance { (best, distance) = (i, d) }
        }
        return best
    }

    /// The height of page `i` rendered `width` pixels wide, rounded down so that a tile never
    /// reaches past the host's own rounding.
    func renderHeight(_ i: Int, width: Int) -> Int {
        Int((max(1, pages[i].height) * CGFloat(width) / max(1, pages[i].width)).rounded(.down))
    }
}
