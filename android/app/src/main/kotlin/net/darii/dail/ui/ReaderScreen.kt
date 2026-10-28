package net.darii.dail.ui

import android.provider.Settings
import androidx.compose.animation.core.animate
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.calculateZoom
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.nestedscroll.NestedScrollConnection
import androidx.compose.ui.input.nestedscroll.NestedScrollSource
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withLink
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Velocity
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlin.math.abs
import kotlin.math.sign

/**
 * The open thread: the toolbar and the messages, collapsed (sender and snippet) or expanded (the
 * body's blocks, attachments, *Links (N)*). A link's tap only sends its action: the presenter
 * shows the full target first ([LinkDialog]).
 *
 * Pulling past the end of the body shows the next conversation's peek, and past its top the
 * previous one's (MISC-18). Only a drag pulls, at half resistance, never a fling's momentum.
 * Released beyond [TURN], the body slides out, the peek slides in, and the conversation's frame
 * replaces it; short of it, the body springs back. TalkBack offers both conversations as custom
 * actions.
 */
@Composable
fun ReaderPane(
    reader: Reader,
    send: (UiEvent) -> Unit,
) {
    val current by rememberUpdatedState(reader)
    val sendNow by rememberUpdatedState(send)
    val context = LocalContext.current
    val still =
        remember {
            Settings.Global.getFloat(context.contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) == 0f
        }
    val turnAt = with(LocalDensity.current) { TURN.toPx() }
    val scope = rememberCoroutineScope()
    // How far the body is pulled past its top (> 0) or its end (< 0), in pixels.
    var pull by remember { mutableFloatStateOf(0f) }
    // The conversation a released pull turns to, while its peek slides in.
    var turning by remember { mutableStateOf<Neighbour?>(null) }
    var height by remember { mutableFloatStateOf(0f) }
    // A new conversation starts at its top.
    val list = remember(reader.thread) { LazyListState() }
    LaunchedEffect(reader.thread) {
        turning = null
        pull = 0f
    }

    fun settle() {
        scope.launch { animate(pull, 0f, animationSpec = spring()) { value, _ -> pull = value } }
    }

    fun release() {
        if (turning != null || pull == 0f) return
        val target = if (pull < 0f) current.next else current.previous
        if (target == null || abs(pull) <= turnAt) return settle()
        turning = target
        scope.launch {
            if (!still) animate(pull, sign(pull) * height, animationSpec = tween(TURN_MS)) { value, _ -> pull = value }
            sendNow(UiEvent.Tap(target.open.action))
            // A turn the presenter ignored (from a frame already replaced) must not leave the body
            // off screen: it comes back.
            delay(STUCK_MS)
            if (turning === target) {
                turning = null
                settle()
            }
        }
    }
    val paging =
        remember {
            object : NestedScrollConnection {
                // While pulled, a drag back shrinks the pull before the body scrolls.
                override fun onPreScroll(
                    available: Offset,
                    source: NestedScrollSource,
                ): Offset {
                    val back = (pull < 0f && available.y > 0f) || (pull > 0f && available.y < 0f)
                    if (source != NestedScrollSource.UserInput || turning != null || !back) return Offset.Zero
                    val used = if (pull < 0f) minOf(available.y, -2 * pull) else maxOf(available.y, -2 * pull)
                    pull += used / 2
                    return Offset(0f, used)
                }

                // What the body could not scroll at an end pulls, towards a conversation there.
                override fun onPostScroll(
                    consumed: Offset,
                    available: Offset,
                    source: NestedScrollSource,
                ): Offset {
                    if (source != NestedScrollSource.UserInput || turning != null) return Offset.Zero
                    val there =
                        when {
                            available.y < 0f -> current.next
                            available.y > 0f -> current.previous
                            else -> null
                        }
                    val against = (pull < 0f && available.y > 0f) || (pull > 0f && available.y < 0f)
                    if (there == null || against) return Offset.Zero
                    pull += available.y / 2
                    return Offset(0f, available.y)
                }

                // The finger lifted: a pull turns or springs back, and its fling is spent.
                override suspend fun onPreFling(available: Velocity): Velocity {
                    if (pull == 0f) return Velocity.Zero
                    release()
                    return available
                }
            }
        }
    // MISC-6: a pinch steps the pane layout (two fingers; one-finger drags stay the body's).
    val pinch =
        Modifier.pointerInput(Unit) {
            awaitEachGesture {
                var zoom = 1f
                var pinching = false
                do {
                    val event = awaitPointerEvent()
                    if (event.changes.count { it.pressed } >= 2) {
                        pinching = true
                        zoom *= event.calculateZoom()
                        event.changes.forEach { it.consume() }
                    }
                } while (event.changes.any { it.pressed })
                if (pinching && (zoom > PINCH_IN || zoom < 1f / PINCH_IN)) sendNow(UiEvent.Pinch(zoom > 1f))
            }
        }
    val actions =
        listOfNotNull(reader.previous, reader.next).map { neighbour ->
            CustomAccessibilityAction(neighbour.open.label) {
                send(UiEvent.Tap(neighbour.open.action))
                true
            }
        }
    Box(
        Modifier
            .fillMaxSize()
            .clipToBounds()
            .onSizeChanged { height = it.height.toFloat() }
            .nestedScroll(paging),
    ) {
        LazyColumn(
            Modifier
                .fillMaxSize()
                .graphicsLayer { translationY = pull }
                .then(pinch)
                .semantics { customActions = actions }
                .padding(horizontal = 16.dp),
            state = list,
        ) {
            item(key = "subject") {
                // MISC-7: the × closes the message; MISC-14: maximize / restore beside it.
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(reader.subject, Modifier.weight(1f), style = MaterialTheme.typography.titleLarge)
                    reader.maximize?.let { ActionButton(it, send) }
                    ActionButton(reader.close, send)
                }
            }
            item(key = "toolbar") { Toolbar(reader, send) }
            items(reader.messages, key = { it.message }) { message ->
                MessageCard(message, send)
                HorizontalDivider()
            }
        }
        // The gap a pull opens shows who is there; while turning it grows into the whole pane.
        val shown =
            turning ?: when {
                pull < 0f -> reader.next
                pull > 0f -> reader.previous
                else -> null
            }
        shown?.let { neighbour ->
            val gap = with(LocalDensity.current) { abs(pull).toDp() }
            Box(
                Modifier
                    .align(if (pull < 0f) Alignment.BottomCenter else Alignment.TopCenter)
                    .fillMaxWidth()
                    .height(gap)
                    .clipToBounds(),
                contentAlignment = Alignment.Center,
            ) { Peek(neighbour, ready = turning == null && abs(pull) > turnAt) }
        }
    }
}

/** A neighbour's peek (MISC-18): which way, then its subject, senders and date as its list row. */
@Composable
private fun Peek(
    neighbour: Neighbour,
    ready: Boolean,
) {
    Column(
        Modifier.padding(horizontal = 16.dp).clearAndSetSemantics {},
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            neighbour.open.icon?.let { Glyph(it) }
            Text(
                if (ready) "Release to open" else neighbour.open.label,
                style = MaterialTheme.typography.labelMedium,
            )
        }
        Text(
            neighbour.subject,
            style = MaterialTheme.typography.titleMedium,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
            textAlign = TextAlign.Center,
        )
        Text(
            "${neighbour.sender} · ${neighbour.date}",
            style = MaterialTheme.typography.bodySmall,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Toolbar(
    reader: Reader,
    send: (UiEvent) -> Unit,
) {
    FlowRow(verticalArrangement = Arrangement.Center) {
        reader.actions.forEach { ActionButton(it, send) }
    }
}

@Composable
private fun MessageCard(
    message: ReaderMessage,
    send: (UiEvent) -> Unit,
) {
    Column(Modifier.fillMaxWidth().padding(vertical = 8.dp)) {
        Column(Modifier.fillMaxWidth().clickable { send(UiEvent.Tap(message.toggle)) }) {
            Text(message.from, fontWeight = FontWeight.Bold)
            if (message.expanded && message.to.isNotEmpty()) {
                Text("to ${message.to}", style = MaterialTheme.typography.bodySmall)
            }
            Text(message.date, style = MaterialTheme.typography.labelSmall)
            if (!message.expanded) Text(message.snippet, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        if (!message.expanded) return@Column
        message.status?.let { Text(it, Modifier.padding(vertical = 8.dp)) }
        message.images?.let { images ->
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(images.text, Modifier.weight(1f), style = MaterialTheme.typography.bodySmall)
                TextButton(onClick = { send(UiEvent.Tap(images.toggle.action)) }) { Text(images.toggle.label) }
            }
        }
        message.body?.let { Blocks(it, send) }
        message.attachments.forEach { file ->
            Row(
                Modifier.fillMaxWidth().clickable { send(UiEvent.Tap(file.open)) },
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Glyph("attach_file")
                Column(Modifier.weight(1f).padding(horizontal = 8.dp)) {
                    Text(file.name, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text("${file.mime} · ${file.size}", style = MaterialTheme.typography.labelSmall)
                }
                ActionButton(Button(icon = "download", label = "Save ${file.name}", action = file.save), send)
            }
        }
        message.saveAll?.let { all -> TextButton(onClick = { send(UiEvent.Tap(all.action)) }) { Text(all.label) } }
        if (message.links.isNotEmpty()) {
            Text(
                "Links (${message.links.size})",
                Modifier.padding(top = 12.dp),
                style = MaterialTheme.typography.labelMedium,
            )
            message.links.forEach { link ->
                Text(
                    link.label,
                    Modifier.fillMaxWidth().clickable { send(UiEvent.Tap(link.action)) }.padding(vertical = 6.dp),
                    color = MaterialTheme.colorScheme.primary,
                    textDecoration = TextDecoration.Underline,
                    style = MaterialTheme.typography.bodySmall,
                )
            }
        }
    }
}

@Composable
private fun Blocks(
    blocks: List<DocBlock>,
    send: (UiEvent) -> Unit,
) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) { blocks.forEach { Block(it, send) } }
}

@Composable
private fun Block(
    block: DocBlock,
    send: (UiEvent) -> Unit,
) {
    val type = MaterialTheme.typography
    when (block) {
        is DocBlock.Paragraph -> {
            Spans(block.spans, type.bodyLarge, send)
        }

        is DocBlock.Heading -> {
            val style =
                when (block.level) {
                    1 -> type.headlineSmall
                    2 -> type.titleLarge
                    else -> type.titleMedium
                }
            Spans(block.spans, style.copy(fontWeight = FontWeight.SemiBold), send)
        }

        is DocBlock.Items -> {
            Column {
                block.items.forEachIndexed { i, item ->
                    Row {
                        Text(if (block.ordered) "${block.start + i}." else "•", Modifier.width(28.dp))
                        Column(Modifier.weight(1f)) { Blocks(item, send) }
                    }
                }
            }
        }

        is DocBlock.Quote -> {
            Row {
                Column(Modifier.width(3.dp).fillMaxHeight().background(MaterialTheme.colorScheme.outlineVariant)) {}
                Column(Modifier.padding(start = 12.dp)) { Blocks(block.blocks, send) }
            }
        }

        is DocBlock.Preformatted -> {
            Column(Modifier.horizontalScroll(rememberScrollState())) {
                block.lines.forEach { Text(it, fontFamily = FontFamily.Monospace, style = type.bodySmall) }
            }
        }

        is DocBlock.Table -> {
            Column(Modifier.horizontalScroll(rememberScrollState())) {
                block.rows.forEach { row ->
                    Row {
                        row.forEach { cell ->
                            val style =
                                if (cell.header) {
                                    type.bodyMedium.copy(
                                        fontWeight = FontWeight.Bold,
                                    )
                                } else {
                                    type.bodyMedium
                                }
                            Column(Modifier.width(140.dp).padding(4.dp)) { Spans(cell.spans, style, send) }
                        }
                    }
                }
            }
        }

        DocBlock.Rule -> {
            HorizontalDivider()
        }

        is DocBlock.Fold -> {
            Text(
                (if (block.expanded) "Hide " else "Show ") + block.label.lowercase(),
                Modifier.clickable { send(UiEvent.Tap(block.toggle)) }.padding(vertical = 4.dp),
                color = MaterialTheme.colorScheme.primary,
                style = type.labelMedium,
            )
            if (block.expanded) Blocks(block.blocks, send)
        }
    }
}

/** Spans, with the message's own pictures (while the sender's show) between runs of text. */
@Composable
private fun Spans(
    spans: List<DocSpan>,
    style: TextStyle,
    send: (UiEvent) -> Unit,
) {
    if (spans.none { it.image != null }) {
        SpanText(spans, style, send)
        return
    }
    Column {
        runs(spans).forEach { run ->
            val image = run.singleOrNull()?.image
            if (image == null) {
                SpanText(run, style, send)
            } else {
                BlobImage(blobRequest("inline", image.blob, cid = image.cid), run[0].text) {
                    SpanText(run, style, send)
                }
            }
        }
    }
}

/** Runs of text spans, each picture span a run of its own. */
private fun runs(spans: List<DocSpan>): List<List<DocSpan>> {
    val out = mutableListOf<MutableList<DocSpan>>()
    spans.forEach { span ->
        val last = out.lastOrNull()
        if (span.image != null || last == null || last.last().image != null) {
            out.add(mutableListOf(span))
        } else {
            last.add(span)
        }
    }
    return out
}

/** Styled spans; a link span carries its action and nothing else of the target. */
@Composable
private fun SpanText(
    spans: List<DocSpan>,
    style: TextStyle,
    send: (UiEvent) -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    val linkStyle = TextLinkStyles(SpanStyle(color = colors.primary, textDecoration = TextDecoration.Underline))
    val text: AnnotatedString =
        buildAnnotatedString {
            spans.forEach { span ->
                val spanStyle =
                    SpanStyle(
                        fontWeight = if (span.bold) FontWeight.Bold else null,
                        fontStyle = if (span.italic) FontStyle.Italic else null,
                        fontFamily = if (span.code) FontFamily.Monospace else null,
                        textDecoration = if (span.strike) TextDecoration.LineThrough else null,
                        background = if (span.hit) colors.tertiaryContainer else Color.Unspecified,
                    )
                val link = span.link
                if (link == null) {
                    withStyle(spanStyle) { append(span.text) }
                } else {
                    val annotation = LinkAnnotation.Clickable("a$link", linkStyle) { send(UiEvent.Tap(link)) }
                    withLink(annotation) { withStyle(spanStyle) { append(span.text) } }
                }
            }
        }
    Text(text, style = style)
}

/** The full target of a tapped link: *Open*, *Copy*, or dismiss. */
@Composable
fun LinkDialog(
    link: LinkSheet,
    send: (UiEvent) -> Unit,
) {
    AlertDialog(
        onDismissRequest = { send(UiEvent.Tap(link.close.action)) },
        title = { Text("Open this link?") },
        text = { Text(link.url, style = MaterialTheme.typography.bodyMedium) },
        confirmButton = { TextButton(onClick = { send(UiEvent.Tap(link.open.action)) }) { Text(link.open.label) } },
        dismissButton = { TextButton(onClick = { send(UiEvent.Tap(link.copy.action)) }) { Text(link.copy.label) } },
    )
}

/** How far a pull past an end must go to turn the conversation (MISC-18). */
private val TURN = 80.dp

/** How long the body takes to slide out when it turns. */
private const val TURN_MS = 250

/** How long a turn waits for the presenter's frame before the body comes back. */
private const val STUCK_MS = 1000L

/** How far a pinch must zoom (or its inverse) to step the layout. */
private const val PINCH_IN = 1.15f
