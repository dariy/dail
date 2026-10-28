package net.darii.dail.ui

import android.provider.Settings
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.draggable
import androidx.compose.foundation.gestures.rememberDraggableState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.nestedscroll.NestedScrollConnection
import androidx.compose.ui.input.nestedscroll.NestedScrollSource
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.Velocity
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import kotlin.math.roundToInt

/**
 * The open message on a compact window: a full-height sheet over the list and the top bar, below
 * the status bar (MISC-17). It slides up on open. A drag down follows the finger, from the grab
 * handle at any time and from the body once the body is scrolled to its top (the body's own scroll
 * comes first, through nested scrolling). Released past a quarter of the height, or flung down, it
 * slides away and sends the reader's close (`close_thread`); otherwise it springs back. With the
 * system's animations off it appears and goes without the slide. Back closes it through the
 * presenter, as before.
 */
@Composable
fun ReaderSheet(
    reader: Reader,
    send: (UiEvent) -> Unit,
) {
    val context = LocalContext.current
    val still =
        remember {
            Settings.Global.getFloat(context.contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) == 0f
        }
    val current by rememberUpdatedState(reader)
    val sendNow by rememberUpdatedState(send)
    val fling = with(LocalDensity.current) { FLING.toPx() }
    BoxWithConstraints(Modifier.fillMaxSize().statusBarsPadding()) {
        val height = constraints.maxHeight.toFloat()
        val offset = remember { Animatable(if (still) 0f else height) }
        val scope = rememberCoroutineScope()
        LaunchedEffect(Unit) { if (!still) offset.animateTo(0f, tween(OPEN_MS)) }

        fun dragBy(dy: Float) {
            scope.launch { offset.snapTo((offset.value + dy).coerceIn(0f, height)) }
        }

        fun release(velocity: Float) {
            scope.launch {
                if (offset.value > height / 4 || velocity > fling) {
                    if (!still) offset.animateTo(height, tween(CLOSE_MS))
                    sendNow(UiEvent.Tap(current.close.action))
                } else {
                    offset.animateTo(0f, spring())
                }
            }
        }
        val nested =
            remember(height) {
                object : NestedScrollConnection {
                    // While the sheet is pulled down, an upward drag lifts it before the body scrolls.
                    override fun onPreScroll(
                        available: Offset,
                        source: NestedScrollSource,
                    ): Offset {
                        if (source != NestedScrollSource.UserInput || available.y >= 0f || offset.value <= 0f) {
                            return Offset.Zero
                        }
                        val used = maxOf(available.y, -offset.value)
                        dragBy(used)
                        return Offset(0f, used)
                    }

                    // A downward drag the body could not use (it is at its top) pulls the sheet.
                    override fun onPostScroll(
                        consumed: Offset,
                        available: Offset,
                        source: NestedScrollSource,
                    ): Offset {
                        if (source != NestedScrollSource.UserInput || available.y <= 0f) return Offset.Zero
                        dragBy(available.y)
                        return Offset(0f, available.y)
                    }

                    override suspend fun onPreFling(available: Velocity): Velocity {
                        if (offset.value <= 0f) return Velocity.Zero
                        release(available.y)
                        return available
                    }
                }
            }
        val drag = rememberDraggableState { dy -> dragBy(dy) }
        Surface(
            Modifier
                .offset { IntOffset(0, offset.value.roundToInt()) }
                .fillMaxSize()
                .nestedScroll(nested),
            shape = RoundedCornerShape(topStart = 16.dp, topEnd = 16.dp),
            shadowElevation = 8.dp,
        ) {
            Column {
                Box(
                    Modifier
                        .fillMaxWidth()
                        .draggable(drag, Orientation.Vertical, onDragStopped = { velocity -> release(velocity) })
                        .padding(vertical = 10.dp),
                    contentAlignment = Alignment.Center,
                ) {
                    Box(
                        Modifier
                            .size(HANDLE_WIDTH, HANDLE_HEIGHT)
                            .background(MaterialTheme.colorScheme.onSurfaceVariant, RoundedCornerShape(2.dp)),
                    )
                }
                ReaderPane(reader, send)
            }
        }
    }
}

private const val OPEN_MS = 250
private const val CLOSE_MS = 200
private val FLING = 1200.dp
private val HANDLE_WIDTH = 32.dp
private val HANDLE_HEIGHT = 4.dp
