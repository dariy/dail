package net.darii.dail.ui

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.util.VelocityTracker
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlin.math.roundToInt

/**
 * The open message on a compact window: a page over the list and the top bar, below the status
 * bar (MISC-18). It slides in from the right. A horizontal drag anywhere on it follows the finger
 * (a block's own horizontal scroll goes first; a vertical drag is the body's scroll). Released past
 * a third of the width, or flung right, it slides away and sends the reader's close
 * (`close_thread`); otherwise it springs back. A frame that drops it over the list (the ×, Back, a
 * thread that goes away) slides it out too; one that puts something else there removes it at once.
 * While a screen that takes the panes' place is up ([covered]: a preview, the palette, a result),
 * it waits off screen, in place. With the system's animations off it appears and goes without the
 * slide.
 */
@Composable
fun ReaderPage(
    reader: Reader?,
    covered: Boolean,
    overList: Boolean,
    send: (UiEvent) -> Unit,
) {
    val still = rememberStill()
    // The frame's reader, or the last one while the page slides out (as `rememberUpdatedState`).
    val kept = remember { mutableStateOf<Reader?>(null) }
    if (reader != null) kept.value = reader
    val shown = kept.value ?: return
    val current by rememberUpdatedState(shown)
    val sendNow by rememberUpdatedState(send)
    val opened by rememberUpdatedState(reader != null)
    val fling = with(LocalDensity.current) { FLING.toPx() }
    BoxWithConstraints(Modifier.fillMaxSize().statusBarsPadding()) {
        val width = constraints.maxWidth.toFloat()
        val slide = remember { Animatable(if (still) 0f else width) }
        val scope = rememberCoroutineScope()
        val open = reader != null
        LaunchedEffect(open) {
            if (open) {
                if (still) slide.snapTo(0f) else slide.animateTo(0f, tween(OPEN_MS))
            } else {
                if (overList && !covered && !still) slide.animateTo(width, tween(CLOSE_MS))
                kept.value = null
            }
        }

        fun settle() {
            scope.launch { if (still) slide.snapTo(0f) else slide.animateTo(0f, spring()) }
        }

        fun release(velocity: Float) {
            scope.launch {
                if (slide.value > width / 3 || velocity > fling) {
                    if (!still) slide.animateTo(width, tween(CLOSE_MS))
                    sendNow(UiEvent.Tap(current.close.action))
                    // A close the presenter ignored (from a frame already replaced) must not leave
                    // the message open off screen: it comes back.
                    delay(STUCK_MS)
                    if (opened) settle()
                } else {
                    settle()
                }
            }
        }
        val swipe =
            Modifier.pointerInput(width) {
                val tracker = VelocityTracker()
                var x = 0f
                detectHorizontalDragGestures(
                    onDragStart = {
                        tracker.resetTracking()
                        x = 0f
                    },
                    onDragEnd = { release(tracker.calculateVelocity().x) },
                    // A drag the system took (a second finger, the Back gesture) never closes.
                    onDragCancel = { settle() },
                ) { change, dx ->
                    change.consume()
                    // The page moves under the finger: track it where the screen is still.
                    x += dx
                    tracker.addPosition(change.uptimeMillis, Offset(x, 0f))
                    scope.launch { slide.snapTo((slide.value + dx).coerceIn(0f, width)) }
                }
            }
        if (!covered) {
            Box(Modifier.fillMaxSize().graphicsLayer { alpha = 1f - slide.value / width }.background(SCRIM))
        }
        val hidden = if (covered) Modifier.clearAndSetSemantics {} else Modifier
        Surface(
            Modifier
                .offset { IntOffset(if (covered) width.roundToInt() else slide.value.roundToInt(), 0) }
                .fillMaxSize()
                .then(hidden)
                .then(swipe),
            shadowElevation = 8.dp,
        ) {
            Box(Modifier.navigationBarsPadding()) { ReaderPane(shown, send) }
        }
    }
}

private const val OPEN_MS = 250
private const val CLOSE_MS = 200
private const val STUCK_MS = 1000L
private val FLING = 1200.dp
private val SCRIM = Color.Black.copy(alpha = 0.32f)
