package net.darii.dail.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.calculateZoom
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.CustomAccessibilityAction
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
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withLink
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp

/**
 * The open thread: the toolbar and the messages, collapsed (sender and snippet) or expanded (the
 * body's blocks, attachments, *Links (N)*). A link's tap only sends its action: the presenter
 * shows the full target first ([LinkDialog]).
 *
 * Beside the list, a horizontal swipe goes to the next (left) or previous (right) conversation
 * (MISC-5); on a page ([swipe] false) a swipe right goes back instead (MISC-18). TalkBack offers
 * both conversations as custom actions. Content wider than the screen scrolls first: its own
 * horizontal scroll consumes the drag, so the swipe only starts at its edge.
 */
@Composable
fun ReaderPane(
    reader: Reader,
    send: (UiEvent) -> Unit,
    swipe: Boolean = true,
) {
    val current by rememberUpdatedState(reader)
    val sendNow by rememberUpdatedState(send)
    val pager =
        if (!swipe) {
            Modifier
        } else {
            Modifier.pointerInput(Unit) {
                var dx = 0f
                detectHorizontalDragGestures(
                    onDragStart = { dx = 0f },
                    onDragEnd = {
                        val target = if (dx < 0) current.next else current.previous
                        if (kotlin.math.abs(dx) > SWIPE_DISTANCE.toPx()) target?.let { sendNow(UiEvent.Tap(it.action)) }
                    },
                ) { change, amount ->
                    change.consume()
                    dx += amount
                }
            }
        }
    // MISC-6: a pinch steps the pane layout (two fingers; one-finger drags stay the swipe's).
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
        listOfNotNull(
            reader.previous?.let { b ->
                CustomAccessibilityAction(b.label) {
                    send(UiEvent.Tap(b.action))
                    true
                }
            },
            reader.next?.let { b ->
                CustomAccessibilityAction(b.label) {
                    send(UiEvent.Tap(b.action))
                    true
                }
            },
        )
    LazyColumn(
        Modifier
            .fillMaxSize()
            .then(pinch)
            .then(pager)
            .semantics { customActions = actions }
            .padding(horizontal = 16.dp),
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

/** How far a horizontal drag must go to change the conversation. */
private val SWIPE_DISTANCE = 72.dp

/** How far a pinch must zoom (or its inverse) to step the layout. */
private const val PINCH_IN = 1.15f
