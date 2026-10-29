package net.darii.dail.ui

import androidx.compose.ui.geometry.Offset
import net.darii.dail.ui.viewer.DocLayout
import net.darii.dail.ui.viewer.Limits
import net.darii.dail.ui.viewer.PageSize
import net.darii.dail.ui.viewer.ViewerState
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** The adapted viewer's geometry (MISC-19), after pdfcraft-android's tests of the same name. */
class ViewerGeometryTest {
    private val a4 = PageSize(595f, 842f)
    private val landscape = PageSize(842f, 595f)
    private val limits = Limits(maxSide = 4096, maxWidth = 16384, tileMax = 1024, tiles = true)

    private fun state(
        pages: List<PageSize>,
        w: Float = 1080f,
        h: Float = 2000f,
    ) = ViewerState(pages, limits).apply {
        gap = 20f
        setViewport(w, h)
    }

    @Test
    fun pagesStackFittedToTheWidestPage() {
        val l = DocLayout(listOf(a4, landscape, a4), 1080f, 2000f, gap = 20f, margin = 0f)
        assertEquals(1080f, l.rects[1].width, 0.01f)
        assertEquals(595f * 1080f / 842f, l.rects[0].width, 0.01f)
        assertEquals(l.rects[0].bottom + 20f, l.rects[1].top, 0.01f)
        assertEquals(l.rects[2].bottom, l.height, 0.01f)
        assertEquals((1080f - l.rects[0].width) / 2f, l.rects[0].left, 0.01f)
    }

    @Test
    fun visibleAndNearestPages() {
        val l = DocLayout(List(100) { a4 }, 1080f, 2000f, 20f, 0f)
        val h = l.rects[0].height
        assertEquals(0..1, l.visible(0f, 2000f))
        assertEquals(50..50, l.visible(l.rects[50].top + 1f, l.rects[50].top + 2f))
        assertEquals(1, l.nearestPage(h + 15f))
        assertEquals(0, l.nearestPage(h + 5f))
    }

    @Test
    fun scrollIsClampedAndSmallDocumentsCentred() {
        val s = state(List(10) { a4 })
        s.scrollBy(0f, 1e9f)
        assertEquals(0f, s.scrollY, 0f)
        s.scrollBy(0f, -1e9f)
        assertEquals(s.layout.height - 2000f, s.scrollY, 0.5f)
        val one = state(listOf(PageSize(100f, 100f)), h = 4000f)
        assertEquals(-(4000f - 1080f) / 2f, one.scrollY, 0.5f)
    }

    @Test
    fun theFocalPointStaysFixedUnderZoom() {
        val s = state(List(3) { a4 })
        val focus = Offset(300f, 700f)
        val before = Offset((focus.x + s.scrollX) / s.zoom, (focus.y + s.scrollY) / s.zoom)
        s.zoomBy(2f, focus)
        val after = Offset((focus.x + s.scrollX) / s.zoom, (focus.y + s.scrollY) / s.zoom)
        assertEquals(before.x, after.x, 0.5f)
        assertEquals(before.y, after.y, 0.5f)
    }

    @Test
    fun zoomStaysWithinItsLimitsAndDoubleTapToggles() {
        val s = state(listOf(a4))
        s.zoomBy(100f, Offset.Zero)
        assertEquals(ViewerState.MAX_ZOOM, s.zoom, 0f)
        s.zoomBy(0.001f, Offset.Zero)
        assertEquals(ViewerState.MIN_ZOOM, s.zoom, 0f)
        assertEquals(ViewerState.DOUBLE_TAP_ZOOM, s.doubleTapTarget(), 0f)
        s.zoomTo(s.doubleTapTarget(), Offset(500f, 500f))
        assertEquals(ViewerState.MIN_ZOOM, s.doubleTapTarget(), 0f)
    }

    @Test
    fun theCurrentPageFollowsTheScroll() {
        val s = state(List(5) { a4 })
        assertEquals(0, s.currentPage)
        s.scrollBy(0f, -(s.layout.rects[3].top - 100f))
        assertEquals(3, s.currentPage)
    }

    @Test
    fun tilesCoverTheScreenAtTheZoomLevel() {
        val s = state(listOf(a4))
        assertEquals(0, s.detailLevel())
        assertTrue(s.requests().all { it.tile == null })
        s.zoomTo(3.9f, Offset.Zero)
        val level = s.detailLevel()
        assertEquals(4, level)
        val width = s.levelWidth(0, level)
        assertEquals(4320, width)
        val tiles = s.requests().filter { it.tile != null }
        assertTrue(tiles.isNotEmpty())
        val height = s.levelHeight(0, width)
        for (r in tiles) {
            val t = r.tile!!
            assertEquals(width, r.width)
            assertTrue(t.w in 1..1024 && t.h in 1..1024)
            assertTrue(t.x + t.w <= width && t.y + t.h <= height)
        }
        // the screen's top-left corner is the page's: its tile is there and comes first
        assertEquals(0, tiles.first().tile!!.x)
        assertEquals(0, tiles.first().tile!!.y)
    }

    @Test
    fun picturesNeverAskForTiles() {
        val s =
            ViewerState(
                listOf(PageSize(4000f, 3000f)),
                limits.copy(tiles = false),
            ).apply { setViewport(1080f, 2000f) }
        s.zoomTo(5f, Offset.Zero)
        assertEquals(0, s.detailLevel())
        assertTrue(s.requests().all { it.tile == null })
    }
}
