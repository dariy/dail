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
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.IconButton
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Snackbar
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarColors
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import net.darii.dail.Screen

/**
 * Draws a frame as it is: the top bar, the panes the layout names, the banner, the snackbar and
 * the floating button, under the modal drawer of small windows. The screens of Wave 4 A3–A6
 * replace the plain panes.
 */
@Composable
fun DailScreen(
    screen: Screen,
    send: (UiEvent) -> Unit,
) {
    when (screen) {
        Screen.Opening -> {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
        }

        is Screen.Failed -> {
            Box(Modifier.fillMaxSize().padding(24.dp), contentAlignment = Alignment.Center) { Text(screen.error) }
        }

        is Screen.Ready -> {
            FrameScreen(screen.frame, send)
        }
    }
}

@Composable
private fun FrameScreen(
    frame: Frame,
    send: (UiEvent) -> Unit,
) {
    BackHandler(enabled = frame.back) { send(UiEvent.Back) }
    ModalDrawer(frame.drawer, send) { FrameScaffold(frame, send) }
    frame.link?.let { LinkDialog(it, send) }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun FrameScaffold(
    frame: Frame,
    send: (UiEvent) -> Unit,
) {
    Scaffold(
        topBar = {
            frame.bulk?.let { bulk ->
                TopAppBar(
                    title = { Text(bulk.title) },
                    navigationIcon = { ActionButton(bulk.close, send) },
                    actions = { bulk.actions.forEach { ActionButton(it, send) } },
                )
                return@Scaffold
            }
            TopAppBar(
                title = {
                    Column {
                        Text(frame.topBar.title, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        frame.topBar.subtitle?.let { Text(it, style = MaterialTheme.typography.labelSmall) }
                    }
                },
                navigationIcon = { frame.topBar.navigation?.let { ActionButton(it, send) } },
                actions = { frame.topBar.actions.forEach { ActionButton(it, send) } },
                colors = chromeBar(LocalChrome.current),
            )
        },
        floatingActionButton = {
            frame.fab?.let { fab ->
                ExtendedFloatingActionButton(onClick = { send(UiEvent.Tap(fab.action)) }) { Text(fab.label) }
            }
        },
        bottomBar = {
            frame.snackbar?.let { snackbar ->
                Snackbar(
                    Modifier.padding(12.dp),
                    action = { snackbar.action?.let { ActionButton(it, send) } },
                ) { Text(snackbar.text) }
            }
        },
    ) { padding ->
        frame.about?.let { about ->
            AboutScreen(about, Modifier.padding(padding))
            return@Scaffold
        }
        frame.preview?.let { preview ->
            PreviewScreen(preview, send, Modifier.padding(padding))
            return@Scaffold
        }
        frame.result?.let { result ->
            ResultScreen(result, send, Modifier.padding(padding))
            return@Scaffold
        }
        frame.setup?.let { sheet ->
            SetupScreen(sheet, send, Modifier.padding(padding))
            return@Scaffold
        }
        frame.palette?.let { sheet ->
            PaletteScreen(sheet, send, Modifier.padding(padding))
            return@Scaffold
        }
        Column(Modifier.padding(padding).fillMaxSize()) {
            frame.banner?.let { BannerCard(it, send) }
            Row(Modifier.fillMaxSize()) {
                frame.layout.panes.forEach { pane ->
                    if (pane == PaneKind.Drawer) {
                        frame.drawer?.let { DrawerPane(it, send) }
                        return@forEach
                    }
                    Box(Modifier.weight(1f).fillMaxSize()) {
                        when (pane) {
                            PaneKind.Drawer -> Unit
                            PaneKind.List -> frame.list?.let { ListPane(it, send) }
                            PaneKind.Reader -> frame.reader?.let { ReaderPane(it, send) }
                            PaneKind.Composer -> frame.composer?.let { ComposerPane(it, send) }
                            PaneKind.Settings -> frame.settings?.let { SettingsPane(it, send) }
                        }
                    }
                }
            }
        }
    }
}

/** The first problem and its fix (`app_front::status`). */
@Composable
private fun BannerCard(
    banner: Banner,
    send: (UiEvent) -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    Surface(Modifier.fillMaxWidth().padding(8.dp), color = colors.errorContainer, shape = MaterialTheme.shapes.medium) {
        Row(Modifier.padding(12.dp), verticalAlignment = Alignment.CenterVertically) {
            Glyph("warning")
            Column(Modifier.weight(1f).padding(horizontal = 12.dp)) {
                Text(banner.text, fontWeight = FontWeight.SemiBold, color = colors.onErrorContainer)
                banner.hint?.let {
                    Text(
                        it,
                        style = MaterialTheme.typography.bodySmall,
                        color = colors.onErrorContainer,
                    )
                }
            }
            banner.action?.let { ActionButton(it, send) }
        }
    }
}

@Composable
internal fun ActionButton(
    button: Button,
    send: (UiEvent) -> Unit,
) {
    val glyph = button.icon?.let { LocalChrome.current.icons[it] }
    if (glyph == null) {
        // The surrounding content colour: the chrome's text in the top bar, not `primary`.
        val colors = ButtonDefaults.textButtonColors(contentColor = LocalContentColor.current)
        TextButton(onClick = { send(UiEvent.Tap(button.action)) }, colors = colors) { Text(button.label) }
        return
    }
    IconButton(
        onClick = { send(UiEvent.Tap(button.action)) },
        modifier = Modifier.semantics { contentDescription = button.label },
    ) {
        Text(glyph, fontFamily = Symbols, fontSize = ICON_SIZE)
    }
}

private val ICON_SIZE = 24.sp

/** The top bar in the theme's chrome group (dark in Dail Light, as on the desktop). */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun chromeBar(chrome: Chrome): TopAppBarColors {
    if (chrome.roles.isEmpty()) return TopAppBarDefaults.topAppBarColors()
    return TopAppBarDefaults.topAppBarColors(
        containerColor = chrome.role("base"),
        titleContentColor = chrome.role("text_strong"),
        navigationIconContentColor = chrome.role("text_strong"),
        actionIconContentColor = chrome.role("text_strong"),
    )
}

/** The version and the licences of the bundled fonts and icons; Back closes it. */
@Composable
private fun AboutScreen(
    about: About,
    modifier: Modifier,
) {
    LazyColumn(modifier.fillMaxSize().padding(horizontal = 16.dp)) {
        item { Text(about.title, style = MaterialTheme.typography.titleLarge) }
        item { Text(about.text, Modifier.padding(vertical = 8.dp)) }
        items(about.licences, key = { it.name }) { licence ->
            Column(Modifier.padding(vertical = 8.dp)) {
                Text(licence.name, style = MaterialTheme.typography.titleMedium)
                Text(licence.text, style = MaterialTheme.typography.labelSmall)
            }
            HorizontalDivider()
        }
    }
}
