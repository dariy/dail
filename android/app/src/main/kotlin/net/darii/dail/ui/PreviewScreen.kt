package net.darii.dail.ui

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.wrapContentSize
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.core.graphics.createBitmap
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import kotlinx.serialization.json.putJsonObject
import net.darii.dail.ui.viewer.Limits
import net.darii.dail.ui.viewer.PageCanvas
import net.darii.dail.ui.viewer.PageImages
import net.darii.dail.ui.viewer.PageSize
import net.darii.dail.ui.viewer.ViewerState
import java.nio.ByteBuffer
import java.nio.ByteOrder

/** Reads `blob` requests (`Host.blob`, IO threads); the activity provides it. */
val LocalBlobs = staticCompositionLocalOf<(JsonObject) -> ByteArray?> { { null } }

/** A `blob` request: `{kind: {blob, key: value}}`. */
fun blobRequest(
    kind: String,
    blob: String,
    part: Int? = null,
    cid: String? = null,
) = buildJsonObject {
    putJsonObject(kind) {
        put("blob", blob)
        part?.let { put("part", it) }
        cid?.let { put("cid", it) }
    }
}

/** Width and height (little-endian `u32`) then RGBA, as `blob`'s `image` and `inline` answer. */
fun rgbaBitmap(bytes: ByteArray): ImageBitmap? {
    if (bytes.size < HEADER) return null
    val header = ByteBuffer.wrap(bytes, 0, HEADER).order(ByteOrder.LITTLE_ENDIAN)
    val width = header.int
    val height = header.int
    if (width <= 0 || height <= 0 || bytes.size - HEADER != width * height * 4) return null
    val bitmap = createBitmap(width, height)
    bitmap.copyPixelsFromBuffer(ByteBuffer.wrap(bytes, HEADER, bytes.size - HEADER))
    return bitmap.asImageBitmap()
}

private const val HEADER = 8

/** What a blob request gave: still loading, or the value (`null` when it failed). */
private sealed interface Loaded<out T> {
    data object Loading : Loaded<Nothing>

    data class Done<T>(
        val value: T?,
    ) : Loaded<T>
}

@Composable
private fun <T> rememberBlob(
    request: JsonObject,
    decode: (ByteArray) -> T?,
): Loaded<T> {
    val blobs = LocalBlobs.current
    val loaded by produceState<Loaded<T>>(Loaded.Loading, request) {
        value = Loaded.Done(withContext(Dispatchers.IO) { blobs(request)?.let(decode) })
    }
    return loaded
}

/** A decoded picture of a stored message; [fallback] while it loads or if it cannot. */
@Composable
fun BlobImage(
    request: JsonObject,
    description: String,
    modifier: Modifier = Modifier,
    fallback: @Composable () -> Unit = {},
) {
    when (val loaded = rememberBlob(request, ::rgbaBitmap)) {
        is Loaded.Done -> {
            loaded.value?.let {
                Image(it, description, modifier, contentScale = ContentScale.Fit)
            } ?: fallback()
        }

        Loaded.Loading -> {
            fallback()
        }
    }
}

/** Runs blob requests one at a time, so a stale page or tile never races a wanted one. */
@OptIn(ExperimentalCoroutinesApi::class)
internal val blobLane = Dispatchers.IO.limitedParallelism(1)

/** [rgbaBitmap] of a blob request on [blobLane]; null when the host or the memory cannot. */
internal suspend fun loadBitmap(
    blobs: (JsonObject) -> ByteArray?,
    request: JsonObject,
): ImageBitmap? =
    withContext(blobLane) {
        try {
            blobs(request)?.let(::rgbaBitmap)
        } catch (_: OutOfMemoryError) {
            // the JNI byte array or the bitmap did not fit: the picture is not shown
            null
        }
    }

/** A picture as a one-page document: pinch, double tap and pan, drawn from the decoded image. */
@Composable
private fun PictureViewer(
    preview: Preview,
    modifier: Modifier,
) {
    val blobs = LocalBlobs.current
    val request = blobRequest("image", preview.blob, part = preview.part)
    val loaded by produceState<Loaded<ImageBitmap>>(Loaded.Loading, request) {
        value = Loaded.Done(loadBitmap(blobs, request))
    }
    when (val picture = loaded) {
        Loaded.Loading -> {
            CircularProgressIndicator(modifier.wrapContentSize())
        }

        is Loaded.Done -> {
            val bitmap = picture.value
            if (bitmap == null) {
                Text("This picture could not be shown.", modifier.padding(12.dp))
                return
            }
            val scope = rememberCoroutineScope()
            val state =
                remember(bitmap) {
                    val size = PageSize(bitmap.width.toFloat(), bitmap.height.toFloat())
                    ViewerState(listOf(size), Limits(maxSide = Int.MAX_VALUE, maxWidth = 0, tileMax = 1, tiles = false))
                }
            val images = remember(bitmap) { PageImages(scope, Int.MAX_VALUE) { bitmap } }
            DisposableEffect(images) { onDispose { images.close() } }
            PageCanvas(
                state,
                images,
                background = MaterialTheme.colorScheme.surfaceContainer,
                paper = Color.Transparent,
                still = rememberStill(),
                modifier = modifier,
                gap = 0.dp,
            )
        }
    }
}

/** A received attachment in the app: a picture, cleaned text, or why it is not shown. */
@Composable
fun PreviewScreen(
    preview: Preview,
    send: (UiEvent) -> Unit,
    modifier: Modifier = Modifier,
) {
    Surface(modifier.fillMaxSize()) {
        Column {
            Row(verticalAlignment = Alignment.CenterVertically) {
                ActionButton(preview.close, send)
                Column(Modifier.weight(1f)) {
                    Text(preview.name, style = MaterialTheme.typography.titleMedium)
                    Text(preview.detail, style = MaterialTheme.typography.labelSmall)
                }
                ActionButton(preview.save, send)
            }
            val body = Modifier.fillMaxWidth().padding(12.dp)
            when (preview.kind) {
                PreviewKind.Image -> {
                    PictureViewer(preview, Modifier.fillMaxSize())
                }

                PreviewKind.Text -> {
                    val request = blobRequest("text", preview.blob, part = preview.part)
                    when (val text = rememberBlob(request) { it.decodeToString() }) {
                        Loaded.Loading -> {
                            CircularProgressIndicator(body.widthIn(max = 48.dp))
                        }

                        is Loaded.Done -> {
                            Text(
                                text.value ?: "The file could not be read.",
                                body.verticalScroll(rememberScrollState()),
                                fontFamily = FontFamily.Monospace,
                                style = MaterialTheme.typography.bodySmall,
                            )
                        }
                    }
                }

                PreviewKind.None -> {
                    preview.note?.let { Text(it, body) }
                }
            }
        }
    }
}
