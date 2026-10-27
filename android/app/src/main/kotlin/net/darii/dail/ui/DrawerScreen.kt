package net.darii.dail.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.NavigationDrawerItem
import androidx.compose.material3.NavigationRail
import androidx.compose.material3.NavigationRailItem
import androidx.compose.material3.PermanentDrawerSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberDrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * The modal drawer of compact and medium windows, over [content] (the panes of the layout). A
 * swipe or a tap on the scrim closes it by sending Back; the presenter then drops it.
 */
@Composable
fun ModalDrawer(
    drawer: Drawer?,
    send: (UiEvent) -> Unit,
    content: @Composable () -> Unit,
) {
    if (drawer == null || !drawer.modal) {
        content()
        return
    }
    val state = rememberDrawerState(DrawerValue.Open)
    LaunchedEffect(state.currentValue) {
        if (state.currentValue == DrawerValue.Closed) send(UiEvent.Back)
    }
    ModalNavigationDrawer(
        drawerState = state,
        drawerContent = { ModalDrawerSheet { DrawerItems(drawer, send) } },
        content = content,
    )
}

/** The drawer as the first pane of a wide window: a sheet, or a rail after `toggle_sidebar`. */
@Composable
fun DrawerPane(
    drawer: Drawer,
    send: (UiEvent) -> Unit,
) {
    if (drawer.rail) {
        DrawerRail(drawer, send)
        return
    }
    // The Scaffold's padding already holds the window insets: the sheet must not add them again,
    // or in landscape (a cutout or the navigation bar at the start) it gets wider and its text
    // starts right of the top bar's (MISC-6).
    PermanentDrawerSheet(Modifier.width(DRAWER_WIDTH), windowInsets = WindowInsets(0)) {
        DrawerItems(drawer, send)
    }
}

@Composable
private fun DrawerItems(
    drawer: Drawer,
    send: (UiEvent) -> Unit,
) {
    LazyColumn(Modifier.padding(horizontal = 12.dp)) {
        drawer.allInboxes?.let { item -> item { DrawerEntry(item, send) } }
        drawer.accounts.forEach { account ->
            item {
                Row(Modifier.padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
                    account.tag?.let { tag ->
                        Box(
                            Modifier
                                .size(TAG_WIDTH, TAG_HEIGHT)
                                .background(LocalChrome.current.tag(tag), RoundedCornerShape(2.dp)),
                        )
                        Spacer(Modifier.width(8.dp))
                    }
                    Column {
                        Text(account.name, style = MaterialTheme.typography.titleSmall, maxLines = 1)
                        if (account.email != account.name) {
                            Text(
                                account.email,
                                style = MaterialTheme.typography.labelSmall,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                    }
                }
            }
            items(account.mailboxes) { item -> DrawerEntry(item, send) }
        }
        item { HorizontalDivider(Modifier.padding(vertical = 8.dp)) }
        items(drawer.footer) { item -> DrawerEntry(item, send) }
    }
}

@Composable
private fun DrawerEntry(
    item: DrawerItem,
    send: (UiEvent) -> Unit,
) {
    NavigationDrawerItem(
        label = { Text(item.label, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        selected = item.selected,
        onClick = { send(UiEvent.Tap(item.action)) },
        icon = { Glyph(item.icon) },
        badge = { if (item.unread > 0) Text(item.unread.toString()) },
        modifier = Modifier.semantics { contentDescription = item.a11y },
    )
}

@Composable
private fun DrawerRail(
    drawer: Drawer,
    send: (UiEvent) -> Unit,
) {
    val items = listOfNotNull(drawer.allInboxes) + drawer.accounts.flatMap { it.mailboxes } + drawer.footer
    NavigationRail(Modifier.fillMaxHeight().width(RAIL_WIDTH), windowInsets = WindowInsets(0)) {
        LazyColumn {
            items(items) { item ->
                NavigationRailItem(
                    selected = item.selected,
                    onClick = { send(UiEvent.Tap(item.action)) },
                    icon = { Glyph(item.icon) },
                    modifier = Modifier.semantics { contentDescription = item.a11y },
                )
                Spacer(Modifier.height(4.dp))
            }
        }
    }
}

/** An icon from the theme's Material Symbols subset, or nothing if it has no glyph. */
@Composable
fun Glyph(name: String) {
    LocalChrome.current.icons[name]?.let { Text(it, fontFamily = Symbols, fontSize = GLYPH_SIZE) }
}

private val DRAWER_WIDTH = 280.dp
private val RAIL_WIDTH = 80.dp
private val TAG_WIDTH = 4.dp
private val TAG_HEIGHT = 16.dp
private val GLYPH_SIZE = 24.sp
