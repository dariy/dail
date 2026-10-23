package net.darii.dail.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.autofill.ContentType
import androidx.compose.ui.semantics.contentType
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp

/**
 * The account setup over the panes: the step's providers, fields, note and error, and its
 * buttons. Each field keeps its own text until the frame's version of it changes; the password
 * goes back as `text` and never comes back in a frame. The input kind picks the keyboard and the
 * autofill hints, so a password manager can fill the address and the password.
 */
@Composable
fun SetupScreen(
    sheet: SetupSheet,
    send: (UiEvent) -> Unit,
    modifier: Modifier = Modifier,
) {
    Surface(modifier.fillMaxSize()) {
        Column(
            Modifier.verticalScroll(rememberScrollState()).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(sheet.title, style = MaterialTheme.typography.headlineSmall)
            Text(sheet.step, style = MaterialTheme.typography.labelMedium)
            sheet.presets.forEach { preset ->
                Row(
                    Modifier.fillMaxWidth().clickable { send(UiEvent.Tap(preset.action)) },
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    RadioButton(selected = preset.selected, onClick = { send(UiEvent.Tap(preset.action)) })
                    Column {
                        Text(preset.label, style = MaterialTheme.typography.bodyLarge)
                        Text(preset.hint, style = MaterialTheme.typography.bodySmall)
                    }
                }
            }
            sheet.fields.forEach { SetupTextField(it, send) }
            sheet.note?.let { Text(it, style = MaterialTheme.typography.bodySmall) }
            sheet.error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
            if (sheet.busy) CircularProgressIndicator()
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TextButton(onClick = { send(UiEvent.Tap(sheet.back.action)) }) { Text(sheet.back.label) }
                Row(Modifier.weight(1f), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)) {
                    sheet.actions.forEachIndexed { index, button ->
                        if (index == sheet.actions.lastIndex) {
                            Button(onClick = { send(UiEvent.Tap(button.action)) }) { Text(button.label) }
                        } else {
                            TextButton(onClick = { send(UiEvent.Tap(button.action)) }) { Text(button.label) }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun SetupTextField(
    field: SetupField,
    send: (UiEvent) -> Unit,
) {
    var text by remember(field.field, field.value.version) { mutableStateOf(field.value.text) }
    val (keyboard, content) =
        when (field.input) {
            InputKind.Email -> {
                KeyboardOptions(keyboardType = KeyboardType.Email, autoCorrectEnabled = false) to
                    (ContentType.EmailAddress + ContentType.Username)
            }

            InputKind.Name -> {
                KeyboardOptions(capitalization = KeyboardCapitalization.Words) to ContentType.PersonFullName
            }

            InputKind.Server -> {
                KeyboardOptions(keyboardType = KeyboardType.Uri, autoCorrectEnabled = false) to null
            }

            InputKind.Password -> {
                KeyboardOptions(keyboardType = KeyboardType.Password, autoCorrectEnabled = false) to
                    ContentType.Password
            }
        }
    OutlinedTextField(
        value = text,
        onValueChange = {
            text = it
            send(UiEvent.Text(field.field, it))
        },
        label = { Text(field.label) },
        placeholder = { if (field.hint.isNotEmpty()) Text(field.hint) },
        singleLine = true,
        keyboardOptions = keyboard,
        visualTransformation =
            if (field.input == InputKind.Password) PasswordVisualTransformation() else VisualTransformation.None,
        modifier =
            Modifier.fillMaxWidth().semantics {
                content?.let { contentType = it }
            },
    )
}
