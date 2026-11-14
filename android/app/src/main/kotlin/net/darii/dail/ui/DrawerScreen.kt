package net.darii.dail.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
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
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.onLongClick
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.withTimeoutOrNull

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
    // Every row reserves the chevron's column when some folder has folders below it (MISC-23).
    val gutter = drawer.accounts.any { account -> account.mailboxes.any { it.fold != null } }
    LazyColumn(Modifier.padding(horizontal = 12.dp)) {
        // Rail ↔ full tree (MISC-11), at the top of the pane.
        drawer.style?.let { style -> item { ActionButton(style, send) } }
        drawer.allInboxes?.let { item -> item { DrawerEntry(item, gutter, send) } }
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
                    Column(Modifier.weight(1f)) {
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
                    account.fold?.let { fold -> Chevron(fold, send) }
                }
            }
            items(account.mailboxes) { item -> DrawerEntry(item, gutter, send) }
        }
        item { HorizontalDivider(Modifier.padding(vertical = 8.dp)) }
        items(drawer.footer) { item -> DrawerEntry(item, gutter, send) }
    }
}

@Composable
private fun DrawerEntry(
    item: DrawerItem,
    gutter: Boolean,
    send: (UiEvent) -> Unit,
) {
    // The folder tree (MISC-10): a chevron before a folder that has folders below it. With a
    // `gutter`, every row keeps the chevron's column, so the titles of one level line up; each
    // level indents half an icon (MISC-23).
    NavigationDrawerItem(
        label = { Text(item.label, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        selected = item.selected,
        onClick = { send(UiEvent.Tap(item.action)) },
        icon = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                val fold = item.fold
                if (fold != null) {
                    Chevron(fold, send)
                } else if (gutter) {
                    Spacer(Modifier.width(CHEVRON_SIZE))
                }
                Glyph(item.icon)
            }
        },
        badge = { if (item.unread > 0) Text(item.unread.toString()) },
        modifier =
            Modifier
                .padding(start = INDENT * item.depth)
                .folderActions(item, send)
                .semantics { contentDescription = item.a11y },
    )
}

/**
 * TCH-4: a long press on an item with `long_press` sends it (the presenter opens the folder's actions); TalkBack
 * offers the same as the long click and the custom action *Folder actions*. The item's own tap still opens it.
 */
private fun Modifier.folderActions(
    item: DrawerItem,
    send: (UiEvent) -> Unit,
): Modifier {
    val action = item.longPress ?: return this
    val name = tr("Folder actions")
    return this
        .pointerInput(action) {
            awaitEachGesture {
                // Seen before the item's own click (the Initial pass), which a long press must not also trigger.
                val down = awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
                val ended =
                    withTimeoutOrNull(viewConfiguration.longPressTimeoutMillis) {
                        var going = true
                        while (going) {
                            val change =
                                awaitPointerEvent(PointerEventPass.Initial).changes.firstOrNull { it.id == down.id }
                            going =
                                change != null &&
                                change.pressed &&
                                !change.isConsumed &&
                                (change.position - down.position).getDistance() <= viewConfiguration.touchSlop
                        }
                        true
                    }
                if (ended == null) {
                    send(UiEvent.LongPress(action))
                    // The release belongs to the long press: consumed, so the item does not also open.
                    do {
                        val event = awaitPointerEvent(PointerEventPass.Initial)
                        event.changes.forEach { it.consume() }
                    } while (event.changes.any { it.pressed })
                }
            }
        }.semantics {
            onLongClick(label = name) {
                send(UiEvent.LongPress(action))
                true
            }
            customActions =
                listOf(
                    CustomAccessibilityAction(name) {
                        send(UiEvent.LongPress(action))
                        true
                    },
                )
        }
}

/**
 * TCH-4: a list of actions on one thing (a folder's) as a bottom sheet: its name, then one 48 dp row per action with
 * its glyph and label. A tap sends the action; dismissing (outside, a swipe down, Back) sends `close`.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ActionSheetView(
    sheet: ActionSheet,
    send: (UiEvent) -> Unit,
) {
    ModalBottomSheet(onDismissRequest = { send(UiEvent.Tap(sheet.close.action)) }) {
        Column(Modifier.navigationBarsPadding().padding(bottom = 8.dp)) {
            Text(
                sheet.title,
                style = MaterialTheme.typography.titleMedium,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp).semantics { heading() },
            )
            sheet.items.forEach { item ->
                Row(
                    Modifier
                        .fillMaxWidth()
                        .heightIn(min = ROW_HEIGHT)
                        .clickable { send(UiEvent.Tap(item.action)) }
                        .padding(horizontal = 24.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Box(Modifier.width(40.dp)) { item.icon?.let { Glyph(it) } }
                    Text(item.label, style = MaterialTheme.typography.bodyLarge)
                }
            }
        }
    }
}

/** Folds or unfolds the folders below a folder or an account. */
@Composable
private fun Chevron(
    fold: Fold,
    send: (UiEvent) -> Unit,
) {
    IconButton(
        onClick = { send(UiEvent.Tap(fold.action)) },
        modifier = Modifier.size(CHEVRON_SIZE).semantics { contentDescription = fold.a11y },
    ) {
        Glyph(if (fold.collapsed) "chevron_right" else "expand_more")
    }
}

@Composable
private fun DrawerRail(
    drawer: Drawer,
    send: (UiEvent) -> Unit,
) {
    val items = listOfNotNull(drawer.allInboxes) + drawer.accounts.flatMap { it.mailboxes } + drawer.footer
    NavigationRail(Modifier.fillMaxHeight().width(RAIL_WIDTH), windowInsets = WindowInsets(0)) {
        LazyColumn {
            drawer.style?.let { style -> item { ActionButton(style, send) } }
            items(items) { item ->
                NavigationRailItem(
                    selected = item.selected,
                    onClick = { send(UiEvent.Tap(item.action)) },
                    icon = { Glyph(item.icon) },
                    modifier = Modifier.folderActions(item, send).semantics { contentDescription = item.a11y },
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

/** Half the 24 dp icon (MISC-23). */
private val INDENT = 12.dp
private val CHEVRON_SIZE = 32.dp
private val RAIL_WIDTH = 80.dp
private val TAG_WIDTH = 4.dp
private val TAG_HEIGHT = 16.dp
private val GLYPH_SIZE = 24.sp
private val ROW_HEIGHT = 48.dp
