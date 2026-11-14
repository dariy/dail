package net.darii.dail.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.PrimaryScrollableTabRow
import androidx.compose.material3.Tab
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.unit.dp

/**
 * The settings page: on a compact window the root's list of sections, or one section's rows; on
 * wider windows the sections as tabs, then the open section's rows under their captions (TCH-6).
 * A choice, an entry or a button sends its action; what it does is the presenter's.
 */
@Composable
fun SettingsPane(
    page: SettingsPage,
    send: (UiEvent) -> Unit,
) {
    Column(Modifier.fillMaxSize()) {
        if (page.tabs.isNotEmpty()) {
            val selected = page.tabs.indexOfFirst { it.selected }.coerceAtLeast(0)
            // The first tab's label, the captions and the rows on the text keyline (MISC-24); a
            // tab's label sits TAB_PADDING inside it.
            PrimaryScrollableTabRow(selectedTabIndex = selected, edgePadding = Keylines.TEXT_START - TAB_PADDING) {
                page.tabs.forEach { tab ->
                    Tab(
                        selected = tab.selected,
                        onClick = { send(UiEvent.Tap(tab.action)) },
                        text = { Text(tab.label) },
                    )
                }
            }
        }
        if (page.sections.isNotEmpty()) {
            LazyColumn(Modifier.fillMaxSize()) {
                itemsIndexed(page.sections, key = { i, entry -> "section:$i:${entry.label}" }) { _, entry ->
                    SettingsEntryView(entry, send)
                }
            }
            return@Column
        }
        LazyColumn(Modifier.fillMaxSize().padding(start = Keylines.TEXT_START, end = Keylines.END)) {
            page.groups.forEach { group ->
                item(key = "group:${group.title}") {
                    Column(Modifier.padding(top = 16.dp, bottom = 4.dp)) {
                        Text(
                            group.title,
                            style = MaterialTheme.typography.titleSmall,
                            color = MaterialTheme.colorScheme.primary,
                        )
                        group.hint?.let { Text(it, style = MaterialTheme.typography.bodySmall) }
                    }
                }
                // By position: labels such as rule names are not unique (SET-5).
                itemsIndexed(group.rows, key = { i, row -> "${group.title}:$i:${row.label}" }) { _, row ->
                    SettingRowView(row, send)
                    HorizontalDivider()
                }
            }
        }
    }
}

/** An entry of the settings root: glyph, label and hint; a folded one is drawn subdued. */
@Composable
private fun SettingsEntryView(
    entry: SettingsEntry,
    send: (UiEvent) -> Unit,
) {
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = ENTRY_HEIGHT)
            .clickable(role = Role.Button) { send(UiEvent.Tap(entry.action)) }
            .alpha(if (entry.folded) FOLDED_ALPHA else 1f)
            .padding(start = Keylines.ICON_START, end = Keylines.END, top = 8.dp, bottom = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // The glyph is decoration: TalkBack reads the label and the hint once.
        Box(Modifier.width(Keylines.TEXT_START - Keylines.ICON_START).clearAndSetSemantics {}) { Glyph(entry.icon) }
        Column {
            Text(
                entry.label,
                style = if (entry.folded) MaterialTheme.typography.bodyMedium else MaterialTheme.typography.bodyLarge,
            )
            entry.hint?.let { Text(it, style = MaterialTheme.typography.bodySmall) }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun SettingRowView(
    row: SettingRow,
    send: (UiEvent) -> Unit,
) {
    Column(Modifier.fillMaxWidth().padding(vertical = 8.dp)) {
        Text(row.label, style = MaterialTheme.typography.bodyLarge)
        if (row.value.isNotEmpty() && row.choices.isEmpty()) {
            Text(row.value, style = MaterialTheme.typography.bodyMedium)
        }
        row.hint?.let { Text(it, style = MaterialTheme.typography.bodySmall) }
        if (row.choices.isNotEmpty() || row.actions.isNotEmpty()) {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                row.choices.forEach { choice ->
                    FilterChip(
                        selected = choice.selected,
                        onClick = { send(UiEvent.Tap(choice.action)) },
                        label = { Text(choice.label) },
                    )
                }
                row.actions.forEach { button ->
                    OutlinedButton(onClick = { send(UiEvent.Tap(button.action)) }) { Text(button.label) }
                }
            }
        }
    }
}

/** The least height of a root entry: a touch target and more. */
private val ENTRY_HEIGHT = 56.dp

/** How much of its colour a folded entry keeps. */
private const val FOLDED_ALPHA = 0.6f

/** Material's horizontal padding inside a tab. */
private val TAB_PADDING = 16.dp
