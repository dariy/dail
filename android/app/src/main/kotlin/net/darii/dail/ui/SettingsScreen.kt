package net.darii.dail.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.PrimaryScrollableTabRow
import androidx.compose.material3.Tab
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

/**
 * The settings page: the sections as tabs, then the open section's rows under their captions. A
 * choice or a button sends its action; what it does is the presenter's.
 */
@Composable
fun SettingsPane(
    page: SettingsPage,
    send: (UiEvent) -> Unit,
) {
    Column(Modifier.fillMaxSize()) {
        val selected = page.tabs.indexOfFirst { it.selected }.coerceAtLeast(0)
        // The first tab's label, the captions and the rows on the text keyline (MISC-24); a tab's
        // label sits TAB_PADDING inside it.
        PrimaryScrollableTabRow(selectedTabIndex = selected, edgePadding = Keylines.TEXT_START - TAB_PADDING) {
            page.tabs.forEach { tab ->
                Tab(
                    selected = tab.selected,
                    onClick = { send(UiEvent.Tap(tab.action)) },
                    text = { Text(tab.label) },
                )
            }
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
                items(group.rows, key = { "${group.title}:${it.label}" }) { row ->
                    SettingRowView(row, send)
                    HorizontalDivider()
                }
            }
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

/** Material's horizontal padding inside a tab. */
private val TAB_PADDING = 16.dp
