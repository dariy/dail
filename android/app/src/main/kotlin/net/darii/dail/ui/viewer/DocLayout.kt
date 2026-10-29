/*
 * Adapted from pdfcraft-android apps/pdfcraft-android/android/app/src/main/java/io/github/artcraftandroid/pdfcraft/ui/viewer/DocLayout.kt@796fa92874ecc92247af95b216b0dad2eee04a08
 * (trimmed to viewing: one continuous column, no edits, comments or selection).
 *
 * Copyright (c) 2026 ArtCraft Team and the PdfCraft contributors
 * MIT License: Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
 * associated documentation files (the "Software"), to deal in the Software without restriction, including without
 * limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the
 * Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:
 * The above copyright notice and this permission notice shall be included in all copies or substantial portions of
 * the Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
 * NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO
 * EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN
 * ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
 * DEALINGS IN THE SOFTWARE.
 */

package net.darii.dail.ui.viewer

import kotlin.math.max

/** A page's displayed size in PDF points (or a picture's in pixels). */
data class PageSize(
    val width: Float,
    val height: Float,
)

/** A rectangle in layout pixels (the document at zoom 1). */
data class LRect(
    val left: Float,
    val top: Float,
    val right: Float,
    val bottom: Float,
) {
    val width get() = right - left
    val height get() = bottom - top

    fun contains(
        x: Float,
        y: Float,
    ) = x in left..right && y in top..bottom
}

/**
 * Where each page is at zoom 1, in layout pixels, for a viewport [viewW] wide: one column scrolled
 * vertically with [gap] between pages, scaled together so the widest fills the width less [margin]
 * on each side. Pure geometry, so it is tested off the device.
 */
class DocLayout(
    val pages: List<PageSize>,
    val viewW: Float,
    val viewH: Float,
    val gap: Float,
    val margin: Float,
) {
    val rects: List<LRect>
    val width: Float
    val height: Float

    init {
        val out = ArrayList<LRect>(pages.size)
        val widest = pages.maxOfOrNull { it.width }?.coerceAtLeast(1f) ?: 1f
        val scale = max(1f, viewW - 2 * margin) / widest
        var y = 0f
        for (p in pages) {
            val w = p.width.coerceAtLeast(1f) * scale
            val h = p.height.coerceAtLeast(1f) * scale
            val x = (viewW - w) / 2f
            out.add(LRect(x, y, x + w, y + h))
            y += h + gap
        }
        width = viewW
        height = if (pages.isEmpty()) viewH else y - gap
        rects = out
    }

    /** Pixels per point of page [i] at zoom 1. */
    fun scaleOf(i: Int): Float = rects[i].width / pages[i].width.coerceAtLeast(1f)

    /** The pages that overlap layout rows [top]..[bottom]. */
    fun visible(
        top: Float,
        bottom: Float,
    ): IntRange {
        if (rects.isEmpty()) return IntRange.EMPTY
        val first = firstAtOrBelow(top)
        var last = first
        while (last + 1 < rects.size && rects[last + 1].top <= bottom) last++
        return first..last
    }

    /** The first page whose bottom is at or below layout row [y] (binary search). */
    private fun firstAtOrBelow(y: Float): Int {
        var lo = 0
        var hi = rects.lastIndex
        while (lo < hi) {
            val mid = (lo + hi) / 2
            if (rects[mid].bottom + gap < y) lo = mid + 1 else hi = mid
        }
        return lo
    }

    /** The page nearest layout row [y]: the one under it, else the closest. */
    fun nearestPage(y: Float): Int {
        if (rects.isEmpty()) return 0
        val i = firstAtOrBelow(y)
        val r = rects[i]
        return when {
            y > r.bottom && i + 1 < rects.size -> if (rects[i + 1].top - y < y - r.bottom) i + 1 else i
            y < r.top && i > 0 -> if (r.top - y > y - rects[i - 1].bottom) i - 1 else i
            else -> i
        }
    }
}
