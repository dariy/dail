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
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.InputChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SuggestionChip
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp

/**
 * The open draft. Each field keeps the IME's state and reports every change as `text` and the
 * focus leaving as `blur`; it takes the frame's text again only when its version changes
 * (`app_mobile::composer`). Writing back, addresses and suggestions are the presenter's.
 */
@Composable
fun ComposerPane(
    composer: Composer,
    send: (UiEvent) -> Unit,
) {
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(composer.title, style = MaterialTheme.typography.titleLarge, modifier = Modifier.weight(1f))
            Text(composer.status, style = MaterialTheme.typography.labelSmall)
            composer.actions.forEach { ActionButton(it, send) }
        }
        composer.problems.forEach {
            Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
        }
        FromRow(composer, send)
        composer.recipients.forEach { RecipientField(it, composer, send) }
        Field(
            key = composer.draft,
            value = composer.subject,
            field = Field.Subject,
            label = "Subject",
            editable = composer.editable,
            singleLine = true,
            send = send,
        )
        Field(
            key = composer.draft,
            value = composer.body,
            field = Field.Body,
            label = "Message",
            editable = composer.editable,
            singleLine = false,
            send = send,
        )
        composer.quote?.let { quote ->
            HorizontalDivider()
            Column(Modifier.fillMaxWidth().clickable { send(UiEvent.Tap(quote.toggle)) }.padding(vertical = 8.dp)) {
                Text(quote.label, style = MaterialTheme.typography.labelMedium)
                quote.header.forEach { Text(it, style = MaterialTheme.typography.bodySmall) }
            }
        }
    }
}

@Composable
private fun FromRow(
    composer: Composer,
    send: (UiEvent) -> Unit,
) {
    var open by remember { mutableStateOf(false) }
    Box {
        val tappable = composer.fromChoices.isNotEmpty()
        Text(
            "From: ${composer.from}",
            Modifier.fillMaxWidth().clickable(enabled = tappable) { open = true }.padding(vertical = 8.dp),
        )
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            composer.fromChoices.forEach { choice ->
                DropdownMenuItem(
                    text = { Text(choice.label, fontWeight = if (choice.selected) FontWeight.Bold else null) },
                    onClick = {
                        open = false
                        if (!choice.selected) send(UiEvent.Tap(choice.action))
                    },
                )
            }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun RecipientField(
    recipients: Recipients,
    composer: Composer,
    send: (UiEvent) -> Unit,
) {
    Column(Modifier.padding(vertical = 4.dp)) {
        FlowRow(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            recipients.chips.forEach { chip ->
                InputChip(
                    selected = false,
                    enabled = composer.editable,
                    onClick = { send(UiEvent.Tap(chip.remove)) },
                    label = { Text(chip.label) },
                    trailingIcon = { Glyph("close") },
                    modifier = Modifier.semantics { contentDescription = "Remove ${chip.label}" },
                )
            }
        }
        Field(
            key = composer.draft,
            value = recipients.input,
            field = recipients.field,
            label = recipients.label,
            editable = composer.editable,
            singleLine = true,
            send = send,
            keyboard = KeyboardOptions(keyboardType = KeyboardType.Email, imeAction = ImeAction.Next),
        )
        FlowRow(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            recipients.suggestions.forEach { suggestion ->
                SuggestionChip(onClick = { send(UiEvent.Tap(suggestion.action)) }, label = { Text(suggestion.label) })
            }
        }
    }
}

/** A text field that keeps its own state until the frame's version of it changes. */
@Composable
private fun Field(
    key: String,
    value: TextValue,
    field: Field,
    label: String,
    editable: Boolean,
    singleLine: Boolean,
    send: (UiEvent) -> Unit,
    keyboard: KeyboardOptions = KeyboardOptions.Default,
) {
    var state by remember(key, field, value.version) {
        mutableStateOf(TextFieldValue(value.text, TextRange(value.text.length)))
    }
    var focused by remember { mutableStateOf(false) }
    TextField(
        value = state,
        onValueChange = { next ->
            val changed = next.text != state.text
            state = next
            if (changed) send(UiEvent.Text(field, next.text))
        },
        label = { Text(label) },
        enabled = editable,
        singleLine = singleLine,
        keyboardOptions = keyboard,
        modifier =
            Modifier.fillMaxWidth().onFocusChanged {
                if (focused && !it.isFocused) send(UiEvent.Blur(field))
                focused = it.isFocused
            },
    )
}
