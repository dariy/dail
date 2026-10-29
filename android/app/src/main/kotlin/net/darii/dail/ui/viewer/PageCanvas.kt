/*
 * Adapted from pdfcraft-android apps/pdfcraft-android/android/app/src/main/java/io/github/artcraftandroid/pdfcraft/ui/viewer/PageCanvas.kt@796fa92874ecc92247af95b216b0dad2eee04a08
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

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.AnimationVector2D
import androidx.compose.animation.core.VectorConverter
import androidx.compose.animation.core.tween
import androidx.compose.animation.splineBasedDecay
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.calculateCentroid
import androidx.compose.foundation.gestures.calculatePan
import androidx.compose.foundation.gestures.calculateZoom
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChanged
import androidx.compose.ui.input.pointer.util.VelocityTracker
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import kotlin.math.abs
import kotlin.math.roundToInt

/**
 * The document view: pages laid out by [state] and drawn from [images] in one canvas, pinch to
 * zoom, drag and fling to scroll, double tap to zoom in or back to fit-width. With [still] (the
 * system's animations off) a double tap jumps instead of animating.
 */
@Composable
fun PageCanvas(
    state: ViewerState,
    images: PageImages,
    background: Color,
    paper: Color,
    still: Boolean,
    modifier: Modifier = Modifier,
    gap: Dp = 8.dp,
) {
    val density = LocalDensity.current
    val scope = rememberCoroutineScope()
    state.gap = with(density) { gap.toPx() }
    val flinger = remember { arrayOfNulls<Job>(1) }

    // keep the renderer working on what is in view
    LaunchedEffect(state, images) {
        snapshotFlow { Triple(state.zoom, state.scrollX to state.scrollY, state.layout) }.collect {
            images.want(state.requests())
        }
    }

    Box(
        modifier
            .fillMaxSize()
            .onSizeChanged { s: IntSize -> state.setViewport(s.width.toFloat(), s.height.toFloat()) }
            .pointerInput(state, still) {
                detectTapGestures(
                    onDoubleTap = { p ->
                        flinger[0]?.cancel()
                        val to = state.doubleTapTarget()
                        if (still) {
                            state.zoomTo(to, p)
                        } else {
                            flinger[0] =
                                scope.launch {
                                    Animatable(
                                        state.zoom,
                                    ).animateTo(to, tween(DOUBLE_TAP_MS)) { state.zoomTo(value, p) }
                                }
                        }
                    },
                    onPress = { flinger[0]?.cancel() },
                )
            }.pointerInput(state) {
                val touchSlop = viewConfiguration.touchSlop
                awaitEachGesture {
                    val down = awaitFirstDown(requireUnconsumed = false)
                    flinger[0]?.cancel()
                    val tracker = VelocityTracker()
                    tracker.addPosition(down.uptimeMillis, down.position)
                    var moved = Offset.Zero
                    var zoomed = 1f
                    var started = false
                    var pinched = false
                    while (true) {
                        val event = awaitPointerEvent()
                        val pressed = event.changes.filter { it.pressed }
                        if (pressed.isEmpty()) break
                        val zoomChange = event.calculateZoom()
                        val pan = event.calculatePan()
                        if (!started) {
                            moved += pan
                            zoomed *= zoomChange
                            started = moved.getDistance() > touchSlop || abs(1f - zoomed) * size.width / 2f > touchSlop
                        }
                        if (started) {
                            if (pressed.size > 1) {
                                pinched = true
                                if (zoomChange != 1f) state.zoomBy(zoomChange, event.calculateCentroid())
                            }
                            if (pan != Offset.Zero) state.scrollBy(pan.x, pan.y)
                            event.changes.forEach { if (it.positionChanged()) it.consume() }
                        }
                        val last = pressed.first()
                        tracker.addPosition(last.uptimeMillis, last.position)
                    }
                    if (!started || pinched) return@awaitEachGesture
                    val v = tracker.calculateVelocity()
                    flinger[0] = scope.launch { fling(state, Offset(v.x, v.y), density.density) }
                }
            },
    ) {
        Canvas(Modifier.fillMaxSize()) {
            // read the image version so arrivals redraw
            images.version
            drawRect(background)
            drawPages(state, images, paper)
        }
    }
}

/** After a drag: fling with decay, stopping at the ends. */
private suspend fun fling(
    state: ViewerState,
    velocity: Offset,
    density: Float,
) {
    if (velocity.getDistance() < 50f) return
    val anim: Animatable<Offset, AnimationVector2D> = Animatable(Offset.Zero, Offset.VectorConverter)
    var last = Offset.Zero
    anim.animateDecay(
        velocity,
        splineBasedDecay(
            androidx.compose.ui.unit
                .Density(density),
        ),
    ) {
        val d = value - last
        last = value
        val left = state.scrollBy(d.x, d.y)
        if (abs(left.x) > 0.5f && abs(left.y) > 0.5f) throw CancellationException("at the edge")
    }
}

private fun DrawScope.drawPages(
    state: ViewerState,
    images: PageImages,
    paper: Color,
) {
    val level = state.detailLevel()
    for (i in state.visiblePages()) {
        val r = state.pageRectOnScreen(i)
        drawRect(paper, topLeft = r.topLeft, size = r.size)
        images.get(ImageKey(i, 0))?.let { drawImageIn(it, r) }
        if (level > 0) {
            val k = r.width / state.levelWidth(i, level)
            for ((index, _) in state.tileGrid(i, level)) {
                val t = images.get(ImageKey(i, level, index)) ?: continue
                val dst =
                    Rect(
                        r.left + t.x * k,
                        r.top + t.y * k,
                        r.left + (t.x + t.bitmap.width) * k,
                        r.top + (t.y + t.bitmap.height) * k,
                    )
                drawImageIn(t, dst)
            }
        }
    }
}

private fun DrawScope.drawImageIn(
    img: PageImage,
    dst: Rect,
) {
    val l = dst.left.roundToInt()
    val t = dst.top.roundToInt()
    drawImage(
        img.bitmap,
        srcOffset = IntOffset.Zero,
        srcSize = IntSize(img.bitmap.width, img.bitmap.height),
        dstOffset = IntOffset(l, t),
        dstSize =
            IntSize(
                (dst.right.roundToInt() - l).coerceAtLeast(1),
                (dst.bottom.roundToInt() - t).coerceAtLeast(1),
            ),
        filterQuality = FilterQuality.Medium,
    )
}

private const val DOUBLE_TAP_MS = 250
