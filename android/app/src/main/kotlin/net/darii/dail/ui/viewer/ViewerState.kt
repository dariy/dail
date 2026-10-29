/*
 * Adapted from pdfcraft-android apps/pdfcraft-android/android/app/src/main/java/io/github/artcraftandroid/pdfcraft/ui/viewer/ViewerState.kt@796fa92874ecc92247af95b216b0dad2eee04a08
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

import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import kotlin.math.ceil
import kotlin.math.floor
import kotlin.math.ln
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow

/** What the host renders, in pixels (`blob`'s `pdf_pages` answer); [tiles] false for a picture. */
data class Limits(
    val maxSide: Int,
    val maxWidth: Int,
    val tileMax: Int,
    val tiles: Boolean,
)

/** A rectangle of a page rendered at a width, in that rendering's pixels. */
data class TileRect(
    val x: Int,
    val y: Int,
    val w: Int,
    val h: Int,
)

/**
 * The view of one document: its layout, zoom and scroll, in screen pixels. Screen = layout ×
 * [zoom] − scroll. Zoom runs from fit-width (1) to [MAX_ZOOM].
 */
@Stable
class ViewerState(
    val pages: List<PageSize>,
    val limits: Limits,
) {
    var viewW by mutableFloatStateOf(0f)
        private set
    var viewH by mutableFloatStateOf(0f)
        private set

    /** Layout spacing, in pixels (set from dp by the canvas). */
    var gap = 0f
    var margin = 0f

    var zoom by mutableFloatStateOf(1f)
        private set
    var scrollX by mutableFloatStateOf(0f)
        private set
    var scrollY by mutableFloatStateOf(0f)
        private set

    private var cachedLayout: DocLayout? = null

    val layout: DocLayout
        get() {
            val c = cachedLayout
            if (c != null && c.viewW == viewW && c.viewH == viewH && c.gap == gap) return c
            return DocLayout(pages, viewW, viewH, gap, margin).also { cachedLayout = it }
        }

    /** The page the reader is on: the one under the top third of the screen. */
    val currentPage: Int
        get() = layout.nearestPage((scrollY + viewH * 0.33f) / zoom)

    /** The viewport changed size: keep the same spot of the document in view. */
    fun setViewport(
        w: Float,
        h: Float,
    ) {
        if (w == viewW && h == viewH) return
        val first = viewW == 0f
        val anchor = if (first) 0 else currentPage
        val within =
            if (!first && layout.rects.isNotEmpty()) {
                val r = layout.rects[anchor]
                ((scrollY / zoom) - r.top) / max(1f, r.height)
            } else {
                0f
            }
        viewW = w
        viewH = h
        if (!first && layout.rects.isNotEmpty()) {
            val r = layout.rects[anchor]
            scrollY = (r.top + within * r.height) * zoom
        }
        clamp()
    }

    /** Scroll by a screen delta (a finger moving by [dx], [dy]); returns what was not consumed. */
    fun scrollBy(
        dx: Float,
        dy: Float,
    ): Offset {
        val ox = scrollX
        val oy = scrollY
        scrollX -= dx
        scrollY -= dy
        clamp()
        return Offset(dx + (scrollX - ox), dy + (scrollY - oy))
    }

    /** Zoom by [factor] about screen point [focus]. */
    fun zoomBy(
        factor: Float,
        focus: Offset,
    ) = zoomTo(zoom * factor, focus)

    /** Zoom to [target] keeping the document point under screen point [focus] in place. */
    fun zoomTo(
        target: Float,
        focus: Offset,
    ) {
        val z = target.coerceIn(MIN_ZOOM, MAX_ZOOM)
        val lx = (focus.x + scrollX) / zoom
        val ly = (focus.y + scrollY) / zoom
        zoom = z
        scrollX = lx * z - focus.x
        scrollY = ly * z - focus.y
        clamp()
    }

    /** A double tap's target: from fit-width in to [DOUBLE_TAP_ZOOM], from deeper back to fit. */
    fun doubleTapTarget(): Float = if (zoom < 1.6f) DOUBLE_TAP_ZOOM else MIN_ZOOM

    /** Keep the document in view: centred when smaller than the screen, else within its edges. */
    fun clamp() {
        val l = layout
        val cw = l.width * zoom
        val ch = l.height * zoom
        scrollX = if (cw <= viewW) -(viewW - cw) / 2f else scrollX.coerceIn(0f, cw - viewW)
        scrollY = if (ch <= viewH) -(viewH - ch) / 2f else scrollY.coerceIn(0f, ch - viewH)
    }

    /** Page [i]'s rectangle on screen. */
    fun pageRectOnScreen(i: Int): Rect {
        val r = layout.rects[i]
        return Rect(
            r.left * zoom - scrollX,
            r.top * zoom - scrollY,
            r.right * zoom - scrollX,
            r.bottom * zoom - scrollY,
        )
    }

    /** Screen pixels per point on page [page] at the current zoom. */
    fun pixelsPerPoint(page: Int): Float = layout.scaleOf(page) * zoom

    /** Visible pages, plus [extra] screens' worth above and below. */
    fun visiblePages(extra: Float = 0f): IntRange {
        val my = viewH * extra
        return layout.visible((scrollY - my) / zoom, (scrollY + viewH + my) / zoom)
    }

    /** The width a page's base image is rendered at: the fit width, within the host's limit. */
    fun baseWidth(i: Int): Int = min(ceil(layout.rects[i].width).toInt().coerceAtLeast(1), limits.maxSide)

    /**
     * What to render for the current view, most wanted first: the visible pages whole, the tiles
     * of the visible area at the current zoom level when zoomed in, then the pages around.
     */
    fun requests(): List<ImageRequest> {
        if (viewW <= 0f || pages.isEmpty()) return emptyList()
        val out = ArrayList<ImageRequest>()
        val visible = visiblePages()
        for (i in visible) out.add(ImageRequest(ImageKey(i, 0), baseWidth(i), null))
        val level = detailLevel()
        if (level > 0) {
            for (i in visible) tilesFor(i, level, out)
        }
        for (i in visiblePages(AROUND)) if (i !in visible) out.add(ImageRequest(ImageKey(i, 0), baseWidth(i), null))
        return out
    }

    /** 0 near fit-width (the base image is sharp enough); then one level per half octave of zoom. */
    fun detailLevel(): Int =
        if (!limits.tiles || zoom < 1.2f) 0 else ceil(ln(zoom) / ln(LEVEL_STEP)).toInt().coerceAtLeast(1)

    /** The width page [i] is rendered at for tiles of [level]. */
    fun levelWidth(
        i: Int,
        level: Int,
    ): Int = min(ceil(layout.rects[i].width * LEVEL_STEP.pow(level)).toInt(), limits.maxWidth)

    /**
     * The height of page [i] rendered [width] wide, rounded down so that a tile never reaches past
     * the host's own rounding.
     */
    fun levelHeight(
        i: Int,
        width: Int,
    ): Int = floor(pages[i].height.coerceAtLeast(1f) * width / pages[i].width.coerceAtLeast(1f)).toInt()

    /** The tiles of page [i] at [level] that cover the screen: a grid of [Limits.tileMax] squares. */
    fun tileGrid(
        i: Int,
        level: Int,
    ): List<Pair<Int, TileRect>> {
        val tile = limits.tileMax
        val pw = levelWidth(i, level)
        val ph = levelHeight(i, pw)
        val screen = pageRectOnScreen(i)
        // screen pixels per pixel of the rendering
        val k = screen.width / pw
        val vx0 = max(0f, -screen.left / k)
        val vy0 = max(0f, -screen.top / k)
        val vx1 = min(pw.toFloat(), (viewW - screen.left) / k)
        val vy1 = min(ph.toFloat(), (viewH - screen.top) / k)
        if (vx1 <= vx0 || vy1 <= vy0 || ph <= 0) return emptyList()
        val cols = (pw + tile - 1) / tile
        val out = ArrayList<Pair<Int, TileRect>>()
        for (ty in (vy0.toInt() / tile)..(min(ph - 1, vy1.toInt()) / tile)) {
            for (tx in (vx0.toInt() / tile)..(min(pw - 1, vx1.toInt()) / tile)) {
                val x = tx * tile
                val y = ty * tile
                val w = min(tile, pw - x)
                val h = min(tile, ph - y)
                if (w > 0 && h > 0) out.add(ty * cols + tx to TileRect(x, y, w, h))
            }
        }
        return out
    }

    private fun tilesFor(
        i: Int,
        level: Int,
        out: MutableList<ImageRequest>,
    ) {
        val width = levelWidth(i, level)
        for ((index, rect) in tileGrid(i, level)) out.add(ImageRequest(ImageKey(i, level, index), width, rect))
    }

    companion object {
        const val MIN_ZOOM = 1f
        const val MAX_ZOOM = 5f
        const val DOUBLE_TAP_ZOOM = 2.5f
        const val LEVEL_STEP = 1.4142135f

        /** Base images are kept ready for this many screens above and below. */
        const val AROUND = 1.5f
    }
}
