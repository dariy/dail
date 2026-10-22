package net.darii.dail.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import net.darii.dail.Screen

/**
 * Draws a frame as it is: the top bar, the panes the layout names, the banner, the snackbar and
 * the floating button. The screens of Wave 4 A3–A6 replace the plain panes.
 */
@Composable
fun DailScreen(screen: Screen, send: (UiEvent) -> Unit) {
    when (screen) {
        Screen.Opening ->
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
        is Screen.Failed ->
            Box(Modifier.fillMaxSize().padding(24.dp), contentAlignment = Alignment.Center) { Text(screen.error) }
        is Screen.Ready -> FrameScreen(screen.frame, send)
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun FrameScreen(frame: Frame, send: (UiEvent) -> Unit) {
    BackHandler(enabled = frame.back) { send(UiEvent.Back) }
    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Column {
                        Text(frame.topBar.title, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        frame.topBar.subtitle?.let { Text(it, style = MaterialTheme.typography.labelSmall) }
                    }
                },
                navigationIcon = { frame.topBar.navigation?.let { ActionButton(it, send) } },
                actions = { frame.topBar.actions.forEach { ActionButton(it, send) } },
            )
        },
        floatingActionButton = {
            frame.fab?.let { fab -> ExtendedFloatingActionButton(onClick = { send(UiEvent.Tap(fab.action)) }) { Text(fab.label) } }
        },
        bottomBar = {
            frame.snackbar?.let { snackbar ->
                Row(Modifier.fillMaxWidth().padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(snackbar.text, Modifier.weight(1f))
                    snackbar.action?.let { ActionButton(it, send) }
                }
            }
        },
    ) { padding ->
        Column(Modifier.padding(padding).fillMaxSize()) {
            frame.banner?.let { banner ->
                Column(Modifier.fillMaxWidth().padding(16.dp)) {
                    Text(banner.text, fontWeight = FontWeight.Bold)
                    banner.hint?.let { Text(it) }
                    banner.action?.let { ActionButton(it, send) }
                }
            }
            Row(Modifier.fillMaxSize()) {
                frame.layout.panes.forEach { pane ->
                    Box(Modifier.weight(1f).fillMaxSize()) {
                        when (pane) {
                            PaneKind.Drawer -> frame.drawer?.let { DrawerPane(it, send) }
                            PaneKind.List -> frame.list?.let { ListPane(it, send) }
                            PaneKind.Reader -> frame.reader?.let { ReaderPane(it, send) }
                            PaneKind.Composer -> frame.composer?.let { ComposerPane(it) }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun ActionButton(button: Button, send: (UiEvent) -> Unit) {
    TextButton(onClick = { send(UiEvent.Tap(button.action)) }) { Text(button.label) }
}

@Composable
private fun DrawerPane(drawer: Drawer, send: (UiEvent) -> Unit) {
    LazyColumn {
        drawer.accounts.forEach { account ->
            item { Text(account.email, Modifier.padding(16.dp), style = MaterialTheme.typography.titleSmall) }
            items(account.mailboxes) { item ->
                Row(
                    Modifier.fillMaxWidth()
                        .clickable { send(UiEvent.Tap(item.action)) }
                        .semantics { contentDescription = item.a11y }
                        .padding(horizontal = 16.dp, vertical = 12.dp),
                ) {
                    Text(item.label, Modifier.weight(1f), fontWeight = if (item.selected) FontWeight.Bold else null)
                    if (item.unread > 0) Text(item.unread.toString())
                }
            }
        }
    }
}

@Composable
private fun ListPane(list: ThreadList, send: (UiEvent) -> Unit) {
    if (list.rows.isEmpty()) {
        list.empty?.let { Text(it, Modifier.padding(24.dp)) }
        return
    }
    LazyColumn {
        items(list.rows, key = { it.thread }) { row ->
            Column(
                Modifier.fillMaxWidth()
                    .clickable { send(UiEvent.Tap(row.action)) }
                    .semantics { contentDescription = row.a11y }
                    .padding(horizontal = 16.dp, vertical = 8.dp),
            ) {
                val weight = if (row.unread) FontWeight.Bold else null
                Row {
                    Text(row.sender, Modifier.weight(1f), fontWeight = weight, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(row.date, style = MaterialTheme.typography.labelSmall)
                }
                Text(row.subject, fontWeight = weight, maxLines = 1, overflow = TextOverflow.Ellipsis)
                if (row.snippet.isNotEmpty()) {
                    Text(row.snippet, maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.bodySmall)
                }
            }
            HorizontalDivider()
        }
    }
}

@Composable
private fun ReaderPane(reader: Reader, send: (UiEvent) -> Unit) {
    LazyColumn(Modifier.padding(horizontal = 16.dp)) {
        item { Text(reader.subject, style = MaterialTheme.typography.titleLarge) }
        item { Row { reader.actions.forEach { ActionButton(it, send) } } }
        items(reader.messages, key = { it.message }) { message ->
            Column(Modifier.fillMaxWidth().clickable { send(UiEvent.Tap(message.toggle)) }.padding(vertical = 8.dp)) {
                Text(message.from, fontWeight = FontWeight.Bold)
                Text(message.date, style = MaterialTheme.typography.labelSmall)
                when {
                    !message.expanded -> Text(message.snippet, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    message.body != null -> message.body.forEach { Text(it) }
                    else -> message.status?.let { Text(it) }
                }
            }
            HorizontalDivider()
        }
    }
}

@Composable
private fun ComposerPane(composer: Composer) {
    Column(Modifier.padding(16.dp)) {
        Text("From: ${composer.from}")
        Text("To: ${composer.to}")
        if (composer.cc.isNotEmpty()) Text("Cc: ${composer.cc}")
        Text(composer.subject, fontWeight = FontWeight.Bold)
        Text(composer.body)
    }
}
