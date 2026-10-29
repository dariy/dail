/*
 * Adapted from pdfcraft-android apps/pdfcraft-android/android/app/src/main/java/io/github/artcraftandroid/pdfcraft/ui/viewer/PageImages.kt@796fa92874ecc92247af95b216b0dad2eee04a08
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

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.ImageBitmap
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.launch

/** What to draw: a whole page ([tile] < 0, at the fit width) or one tile of it at a zoom [level]. */
data class ImageKey(
    val page: Int,
    val level: Int,
    val tile: Int = -1,
) {
    val isBase get() = tile < 0
}

/** A rendered image and where it goes: a page, or a tile's origin in its rendering's pixels. */
class PageImage(
    val key: ImageKey,
    val bitmap: ImageBitmap,
    val x: Int,
    val y: Int,
) {
    val bytes: Int get() = bitmap.width * bitmap.height * 4
}

/** A request: the key, the width the page is rendered at, and the tile of it, if any. */
data class ImageRequest(
    val key: ImageKey,
    val width: Int,
    val tile: TileRect?,
)

/**
 * The rendered pages and tiles of one document, and the worker that renders what the view asks
 * for, most wanted first, through [render] (which runs on its own lane and answers null when the
 * host cannot). The view calls [want] with every frame's list; whatever is no longer wanted when
 * the worker gets to it is skipped. Images are kept up to [budgetBytes], least recently used
 * going first; tiles off screen are dropped.
 */
class PageImages(
    scope: CoroutineScope,
    private val budgetBytes: Int,
    private val render: suspend (ImageRequest) -> ImageBitmap?,
) {
    private val lock = Any()
    private val cache = LinkedHashMap<ImageKey, PageImage>(64, 0.75f, true)
    private var wanted: List<ImageRequest> = emptyList()
    private val failed = HashSet<ImageKey>()
    private var bytes = 0
    private val signal = Channel<Unit>(Channel.CONFLATED)

    /** Bumped whenever an image arrives: the canvas reads it to redraw. */
    var version by mutableIntStateOf(0)
        private set

    /** Bumped when a whole page fails: the screen shows the error. */
    var failures by mutableIntStateOf(0)
        private set

    private val job = SupervisorJob(scope.coroutineContext[Job])

    init {
        scope.launch(job) { work() }
    }

    /** Stop the worker and drop the images. */
    fun close() {
        job.cancel()
        synchronized(lock) {
            cache.clear()
            bytes = 0
        }
    }

    /** The images the view wants now, most wanted first. Tiles no longer wanted are dropped. */
    fun want(requests: List<ImageRequest>) {
        synchronized(lock) {
            wanted = requests
            val keep = requests.mapTo(HashSet()) { it.key }
            val it = cache.entries.iterator()
            while (it.hasNext()) {
                val e = it.next()
                if (!e.key.isBase && e.key !in keep) {
                    bytes -= e.value.bytes
                    it.remove()
                }
            }
        }
        signal.trySend(Unit)
    }

    fun get(key: ImageKey): PageImage? = synchronized(lock) { cache[key] }

    /** Whether a whole page could not be rendered. */
    fun failed(page: Int): Boolean = synchronized(lock) { ImageKey(page, 0) in failed }

    private fun next(): ImageRequest? =
        synchronized(lock) {
            wanted.firstOrNull { it.key !in cache && it.key !in failed }
        }

    private suspend fun work() {
        while (true) {
            val r = next()
            if (r == null) {
                signal.receive()
                continue
            }
            val bitmap = render(r)
            synchronized(lock) {
                if (bitmap == null) {
                    failed.add(r.key)
                } else {
                    val image = PageImage(r.key, bitmap, r.tile?.x ?: 0, r.tile?.y ?: 0)
                    cache.put(r.key, image)?.let { bytes -= it.bytes }
                    bytes += image.bytes
                    evict()
                }
            }
            if (bitmap == null && r.key.isBase) failures++
            if (bitmap != null) version++
        }
    }

    /** Drop least recently used images over budget, never the ones wanted now. */
    private fun evict() {
        if (bytes <= budgetBytes) return
        val keep = wanted.mapTo(HashSet()) { it.key }
        val it = cache.entries.iterator()
        while (bytes > budgetBytes && it.hasNext()) {
            val e = it.next()
            if (e.key in keep) continue
            bytes -= e.value.bytes
            it.remove()
        }
    }
}
