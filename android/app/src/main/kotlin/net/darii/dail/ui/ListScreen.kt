package net.darii.dail.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.launch

/**
 * The thread list: `total` items, of which the frame carries a window (`offset`, `rows`); the
 * others are placeholders until the window moves there. Kotlin reports what is visible
 * (`scrolled`) and the presenter sends the rows around it.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ListPane(
    list: ThreadList,
    send: (UiEvent) -> Unit,
) {
    PullToRefreshBox(
        isRefreshing = list.refreshing,
        onRefresh = { send(UiEvent.Refresh) },
        modifier = Modifier.fillMaxSize(),
    ) {
        if (list.total == 0) {
            EmptyList(list.server ?: list.empty)
            return@PullToRefreshBox
        }
        val state = rememberLazyListState()
        ReportScrolling(state, send)
        LazyColumn(Modifier.fillMaxSize(), state = state) {
            list.server?.let { server ->
                item(key = "server") {
                    Text(
                        server,
                        Modifier.fillMaxWidth().padding(
                            start = Keylines.TEXT_START,
                            end = Keylines.END,
                            top = 6.dp,
                            bottom = 6.dp,
                        ),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
            items(
                count = list.total,
                key = { index -> list.rows.getOrNull(index - list.offset)?.thread ?: "placeholder-$index" },
            ) { index ->
                val thread = list.rows.getOrNull(index - list.offset)
                if (thread == null) {
                    Placeholder(list.density)
                } else {
                    SwipeRow(thread, send) { ThreadRow(thread, list.density, send) }
                }
                HorizontalDivider()
            }
        }
    }
}

/** Sends `scrolled` whenever the first visible row or the number of visible rows changes. */
@Composable
private fun ReportScrolling(
    state: LazyListState,
    send: (UiEvent) -> Unit,
) {
    LaunchedEffect(state) {
        snapshotFlow { state.firstVisibleItemIndex to state.layoutInfo.visibleItemsInfo.size }
            .distinctUntilChanged()
            .collect { (first, count) -> send(UiEvent.Scrolled(first, count)) }
    }
}

/** The list's empty state, scrollable so that pull to refresh still works. */
@Composable
private fun EmptyList(text: String?) {
    Column(
        Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(start = Keylines.TEXT_START, end = Keylines.TEXT_START, top = 32.dp, bottom = 32.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Spacer(Modifier.height(96.dp))
        Glyph("inbox")
        Spacer(Modifier.height(8.dp))
        text?.let { Text(it, textAlign = TextAlign.Center, style = MaterialTheme.typography.bodyMedium) }
    }
}

/** One thread, in the frame's density: one line, two, or three with the snippet. */
@Composable
fun ThreadRow(
    row: Row,
    density: Density,
    send: (UiEvent) -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = MaterialTheme.colorScheme
    val background =
        when {
            row.checked -> colors.secondaryContainer
            row.selected -> colors.primaryContainer
            else -> colors.surface
        }
    val weight = if (row.unread) FontWeight.SemiBold else FontWeight.Normal
    val subject = row.marks?.let { marked(it.subject) } ?: AnnotatedString(row.subject)
    val vertical = if (density == Density.Compact) 6.dp else 10.dp
    Row(
        modifier
            .combinedClickable(
                onClick = { send(UiEvent.Tap(row.action)) },
                onLongClick = { send(UiEvent.LongPress(row.longPress)) },
            ).fillMaxWidth()
            .background(background)
            .semantics(mergeDescendants = true) { contentDescription = row.a11y }
            .padding(end = Keylines.END, top = vertical, bottom = vertical),
        verticalAlignment = if (density == Density.Compact) Alignment.CenterVertically else Alignment.Top,
    ) {
        // The colour bar at the edge, the dot or check box centred in the icon column, the text on
        // the text keyline (MISC-20).
        Box(Modifier.width(Keylines.TEXT_START)) {
            Tag(row.tag)
            Box(
                Modifier
                    .padding(start = Keylines.ICON_START)
                    .size(Keylines.ICON_SIZE),
                contentAlignment = Alignment.Center,
            ) {
                if (row.checked) {
                    Glyph("check_box")
                } else if (row.unread) {
                    Box(Modifier.size(8.dp).background(colors.secondary, CircleShape))
                }
            }
        }
        Column(Modifier.weight(1f)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    row.sender + if (row.messages > 1) "  ${row.messages}" else "",
                    Modifier.weight(if (density == Density.Compact) 0.4f else 1f),
                    fontWeight = weight,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (density == Density.Compact) {
                    Text(
                        subject,
                        Modifier.weight(0.6f).padding(start = 8.dp),
                        fontWeight = weight,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                Marks(row)
                Text(row.date, Modifier.padding(start = 8.dp), style = MaterialTheme.typography.labelSmall)
            }
            if (density != Density.Compact) {
                Text(subject, fontWeight = weight, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
            if (density == Density.Preview && row.snippet.isNotEmpty()) {
                Text(
                    row.marks?.let { marked(it.snippet) } ?: AnnotatedString(row.snippet),
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    style = MaterialTheme.typography.bodySmall,
                    color = colors.onSurfaceVariant,
                )
            }
        }
    }
}

/**
 * A row that swipes: towards the start edge (left) archives, towards the end edge (right)
 * trashes. The action runs only once the finger is lifted past half the row and the row has
 * settled off-screen (`onDismiss`); dragging back under half before lifting cancels it. The row
 * then springs back; the next frame drops it.
 *
 * The swipe state is remembered across frames, but action IDs are per frame (the presenter
 * forgets the old ones), so the callback reads the row of the current frame (MISC-1): with the
 * row captured at the first composition, every swipe after the first new frame sent a stale ID
 * and did nothing.
 */
@Composable
private fun SwipeRow(
    row: Row,
    send: (UiEvent) -> Unit,
    content: @Composable () -> Unit,
) {
    val current by rememberUpdatedState(row)
    val sendNow by rememberUpdatedState(send)
    val scope = rememberCoroutineScope()
    val state = rememberSwipeToDismissBoxState()
    SwipeToDismissBox(
        state = state,
        backgroundContent = { SwipeBackground(state.dismissDirection) },
        onDismiss = { value ->
            when (value) {
                SwipeToDismissBoxValue.EndToStart -> sendNow(UiEvent.Swipe(current.swipeStart))
                SwipeToDismissBoxValue.StartToEnd -> sendNow(UiEvent.Swipe(current.swipeEnd))
                SwipeToDismissBoxValue.Settled -> Unit
            }
            scope.launch { state.reset() }
        },
        content = { content() },
    )
}

@Composable
private fun SwipeBackground(direction: SwipeToDismissBoxValue) {
    val colors = MaterialTheme.colorScheme
    val (icon, color, alignment) =
        when (direction) {
            SwipeToDismissBoxValue.EndToStart -> Triple("archive", colors.secondaryContainer, Alignment.CenterEnd)
            SwipeToDismissBoxValue.StartToEnd -> Triple("delete", colors.errorContainer, Alignment.CenterStart)
            SwipeToDismissBoxValue.Settled -> Triple(null, Color.Transparent, Alignment.Center)
        }
    Box(Modifier.fillMaxSize().background(color).padding(horizontal = 24.dp), contentAlignment = alignment) {
        icon?.let { Glyph(it) }
    }
}

/** The paper clip, the star and the pin. */
@Composable
private fun Marks(row: Row) {
    if (row.attachments) Box(Modifier.padding(start = 4.dp)) { Glyph("attach_file") }
    if (row.starred) Box(Modifier.padding(start = 4.dp)) { Glyph("star") }
    if (row.pinned) Box(Modifier.padding(start = 4.dp)) { Glyph("push_pin") }
}

/** The account's colour bar, with two accounts or more. */
@Composable
private fun Tag(tag: Int?) {
    val color = tag?.let { LocalChrome.current.tag(it) } ?: Color.Transparent
    Box(
        Modifier
            .size(TAG_WIDTH, TAG_HEIGHT)
            .background(color, RoundedCornerShape(2.dp)),
    )
}

/** A row outside the frame's window, until the window moves there. */
@Composable
private fun Placeholder(density: Density) {
    val height =
        when (density) {
            Density.Compact -> 36.dp
            Density.Cozy -> 60.dp
            Density.Preview -> 96.dp
        }
    Box(Modifier.fillMaxWidth().height(height).background(MaterialTheme.colorScheme.surface))
}

private val TAG_WIDTH = 3.dp
private val TAG_HEIGHT = 32.dp
