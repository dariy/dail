package net.darii.dail.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp

/**
 * The command palette over the panes: the query field (its text goes back as `text` on the
 * `query` field), the second step's question, and the rows under their captions. Ranking, steps
 * and what a row does are the presenter's.
 */
@Composable
fun PaletteScreen(
    sheet: PaletteSheet,
    send: (UiEvent) -> Unit,
    modifier: Modifier = Modifier,
) {
    Surface(modifier.fillMaxSize()) {
        Column {
            Row(verticalAlignment = Alignment.CenterVertically) {
                ActionButton(sheet.close, send)
                QueryField(sheet, send, Modifier.weight(1f))
            }
            sheet.prompt?.let {
                Text(
                    it,
                    Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                    style = MaterialTheme.typography.titleSmall,
                )
            }
            LazyColumn(Modifier.fillMaxSize()) {
                sheet.sections.forEach { section ->
                    if (section.title.isNotEmpty()) {
                        item(key = "caption-${section.title}") {
                            Text(
                                section.title,
                                Modifier.padding(start = 16.dp, top = 12.dp, bottom = 4.dp),
                                style = MaterialTheme.typography.labelMedium,
                                color = MaterialTheme.colorScheme.primary,
                            )
                        }
                    }
                    items(section.rows, key = { it.action }) { row -> PaletteRowItem(row, send) }
                }
            }
        }
    }
}

@Composable
private fun QueryField(
    sheet: PaletteSheet,
    send: (UiEvent) -> Unit,
    modifier: Modifier,
) {
    val value = sheet.query
    var state by remember(value.version) { mutableStateOf(TextFieldValue(value.text, TextRange(value.text.length))) }
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { focus.requestFocus() }
    TextField(
        value = state,
        onValueChange = { next ->
            val changed = next.text != state.text
            state = next
            if (changed) send(UiEvent.Text(Field.Query, next.text))
        },
        placeholder = { Text(sheet.hint) },
        singleLine = true,
        modifier = modifier.focusRequester(focus),
    )
}

@Composable
private fun PaletteRowItem(
    row: PaletteRow,
    send: (UiEvent) -> Unit,
) {
    Row(
        Modifier
            .fillMaxWidth()
            .clickable {
                send(
                    UiEvent.Tap(row.action),
                )
            }.padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(row.title, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (row.detail.isNotEmpty()) {
                Text(
                    row.detail,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        row.keys?.let { Text(it, style = MaterialTheme.typography.labelSmall, fontFamily = FontFamily.Monospace) }
    }
}

/**
 * A query's result under the note that it is untrusted: sections of labelled fields and rows in
 * the list's style, or the raw JSON after *Raw data* (MISC-22).
 */
@Composable
fun ResultScreen(
    result: ResultView,
    send: (UiEvent) -> Unit,
    modifier: Modifier = Modifier,
) {
    Surface(modifier.fillMaxSize()) {
        Column {
            Row(verticalAlignment = Alignment.CenterVertically) {
                ActionButton(result.close, send)
                Text(result.title, Modifier.weight(1f), style = MaterialTheme.typography.titleMedium)
                ActionButton(result.raw, send)
            }
            Text(
                result.note,
                Modifier.fillMaxWidth().background(MaterialTheme.colorScheme.tertiaryContainer).padding(12.dp),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onTertiaryContainer,
            )
            HorizontalDivider()
            LazyColumn(Modifier.fillMaxSize()) {
                result.sections.forEach { section ->
                    section.title?.let { title ->
                        item {
                            Text(
                                title,
                                Modifier.padding(start = Keylines.TEXT_START, top = 16.dp, bottom = 4.dp),
                                style = MaterialTheme.typography.titleSmall,
                                color = MaterialTheme.colorScheme.primary,
                            )
                        }
                    }
                    items(section.fields) { field -> ResultFieldRow(field) }
                    items(section.rows) { row ->
                        ResultRowView(row, send)
                        HorizontalDivider()
                    }
                }
                items(result.lines) { line ->
                    Text(
                        line,
                        Modifier.padding(horizontal = 12.dp),
                        fontFamily = FontFamily.Monospace,
                        style = MaterialTheme.typography.bodySmall,
                    )
                }
            }
        }
    }
}

@Composable
private fun ResultFieldRow(field: ResultField) {
    Row(Modifier.fillMaxWidth().padding(start = Keylines.TEXT_START, end = Keylines.END, top = 4.dp, bottom = 4.dp)) {
        if (field.label.isNotEmpty()) {
            Text(
                field.label,
                Modifier.width(RESULT_LABEL_WIDTH),
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Text(field.value, style = MaterialTheme.typography.bodyMedium)
    }
}

/** A result's row, drawn as the list draws a thread: the marks in the icon column, the text on the keyline. */
@Composable
private fun ResultRowView(
    row: ResultRow,
    send: (UiEvent) -> Unit,
) {
    val tap = row.action
    val unread = "unread" in row.marks
    Row(
        Modifier
            .fillMaxWidth()
            .then(if (tap != null) Modifier.clickable { send(UiEvent.Tap(tap)) } else Modifier)
            .padding(end = Keylines.END, top = 10.dp, bottom = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.width(Keylines.TEXT_START), contentAlignment = Alignment.Center) {
            when {
                "failed" in row.marks -> Glyph("close")
                "passed" in row.marks -> Glyph("check")
                "pending" in row.marks -> Glyph("schedule")
                unread -> Box(Modifier.size(8.dp).background(MaterialTheme.colorScheme.secondary, CircleShape))
            }
        }
        Column(Modifier.weight(1f)) {
            Text(
                row.title,
                fontWeight = if (unread) FontWeight.SemiBold else FontWeight.Normal,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            row.detail?.let {
                Text(it, maxLines = 2, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.bodySmall)
            }
        }
        if ("attachment" in row.marks) Glyph("attach_file")
        if ("starred" in row.marks) Glyph("star")
        if ("pinned" in row.marks) Glyph("push_pin")
        row.meta?.let { Text(it, Modifier.padding(start = 8.dp), style = MaterialTheme.typography.labelSmall) }
    }
}

private val RESULT_LABEL_WIDTH = 112.dp

/** Text with its search words marked (`app_mobile::frame::Piece`). */
@Composable
fun marked(pieces: List<Piece>): AnnotatedString {
    val hit = SpanStyle(fontWeight = FontWeight.Bold, background = MaterialTheme.colorScheme.tertiaryContainer)
    return buildAnnotatedString {
        pieces.forEach { piece -> if (piece.hit) withStyle(hit) { append(piece.text) } else append(piece.text) }
    }
}
