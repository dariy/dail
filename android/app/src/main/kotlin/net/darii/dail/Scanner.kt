package net.darii.dail

import android.os.Handler
import android.os.Looper
import androidx.activity.compose.BackHandler
import androidx.camera.compose.CameraXViewfinder
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageProxy
import androidx.camera.core.Preview
import androidx.camera.core.SurfaceRequest
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.lifecycle.awaitInstance
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.LocalLifecycleOwner
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.awaitCancellation
import net.darii.dail.ui.ScanFailure
import net.darii.dail.ui.tr
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean

/** How the scanner ended: exactly one of these, once. */
sealed interface ScanEnd {
    /** The first decoded QR code's text. */
    data class Scanned(
        val text: String,
    ) : ScanEnd

    /** Back or *Cancel*. */
    data object Cancelled : ScanEnd

    /** The camera could not run. */
    data class Failed(
        val reason: ScanFailure,
    ) : ScanEnd
}

/**
 * The in-app QR code scanner (SRV-QR3), full screen over the frame: CameraX's preview and an
 * analysis stream whose Y planes go to [QrDecoder]. The first decoded text ends it; nothing here
 * looks at what the text says (Rust's `scanned` does). The camera permission was granted before
 * this is composed; leaving the composition unbinds the camera.
 */
@Composable
fun ScannerScreen(onEnd: (ScanEnd) -> Unit) {
    val context = LocalContext.current
    val owner = LocalLifecycleOwner.current
    val end by rememberUpdatedState(onEnd)
    val done = remember { AtomicBoolean(false) }
    var request by remember { mutableStateOf<SurfaceRequest?>(null) }
    val finish: (ScanEnd) -> Unit = { result -> if (done.compareAndSet(false, true)) end(result) }
    BackHandler { finish(ScanEnd.Cancelled) }
    LaunchedEffect(Unit) {
        val provider =
            runCatching { ProcessCameraProvider.awaitInstance(context) }.getOrElse {
                finish(ScanEnd.Failed(ScanFailure.Failed))
                return@LaunchedEffect
            }
        val selector =
            listOf(CameraSelector.DEFAULT_BACK_CAMERA, CameraSelector.DEFAULT_FRONT_CAMERA).firstOrNull {
                runCatching { provider.hasCamera(it) }.getOrDefault(false)
            }
        if (selector == null) {
            finish(ScanEnd.Failed(ScanFailure.NoCamera))
            return@LaunchedEffect
        }
        val preview = Preview.Builder().build().apply { setSurfaceProvider { request = it } }
        val analysis =
            ImageAnalysis
                .Builder()
                .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                .build()
        val worker = Executors.newSingleThreadExecutor { Thread(it, "dail-scan").apply { isDaemon = true } }
        val main = Handler(Looper.getMainLooper())
        val decoder = QrDecoder()
        analysis.setAnalyzer(worker) { image ->
            val text = image.use { if (done.get()) null else decode(decoder, it) }
            if (text != null) main.post { finish(ScanEnd.Scanned(text)) }
        }
        try {
            try {
                provider.bindToLifecycle(owner, selector, preview, analysis)
            } catch (e: CancellationException) {
                throw e
            } catch (_: Exception) {
                finish(ScanEnd.Failed(ScanFailure.Failed))
            }
            awaitCancellation()
        } finally {
            analysis.clearAnalyzer()
            provider.unbind(preview, analysis)
            worker.shutdown()
        }
    }
    // A Surface takes the touches, so nothing reaches the frame underneath.
    Surface(Modifier.fillMaxSize(), color = Color.Black, contentColor = Color.White) {
        Box(Modifier.fillMaxSize()) {
            request?.let { CameraXViewfinder(surfaceRequest = it, modifier = Modifier.fillMaxSize()) }
            Column(
                Modifier
                    .align(Alignment.TopCenter)
                    .fillMaxWidth()
                    .background(Color.Black.copy(alpha = SCRIM))
                    .safeDrawingPadding()
                    .padding(16.dp),
            ) {
                Text(tr("Scan QR code"), style = MaterialTheme.typography.titleLarge)
                Text(tr("Point the camera at the QR code dail shows on the server."))
            }
            Box(
                Modifier
                    .align(Alignment.BottomCenter)
                    .fillMaxWidth()
                    .background(Color.Black.copy(alpha = SCRIM))
                    .safeDrawingPadding()
                    .padding(8.dp),
                contentAlignment = Alignment.Center,
            ) {
                TextButton(onClick = { finish(ScanEnd.Cancelled) }) { Text(tr("Cancel"), color = Color.White) }
            }
        }
    }
}

/** A frame's Y plane (YUV_420_888: one byte per pixel) through [decoder]. */
private fun decode(
    decoder: QrDecoder,
    image: ImageProxy,
): String? {
    val plane = image.planes.firstOrNull() ?: return null
    if (plane.pixelStride != 1) return null
    val buffer = plane.buffer
    val bytes = ByteArray(buffer.remaining())
    buffer.get(bytes)
    return decoder.decode(bytes, image.width, image.height, plane.rowStride)
}

private const val SCRIM = 0.6f
