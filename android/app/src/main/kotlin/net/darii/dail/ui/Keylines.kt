package net.darii.dail.ui

import androidx.compose.ui.unit.dp

/**
 * The top bar's two keylines, shared by the panes (MISC-20): the icon column (the menu icon,
 * [ICON_START] to [ICON_END] from the edge) and the text keyline ([TEXT_START], the title's start).
 * In multi-pane windows they count from the pane's edge.
 */
object Keylines {
    val ICON_START = 16.dp
    val ICON_SIZE = 24.dp
    val ICON_END = ICON_START + ICON_SIZE
    val TEXT_START = 56.dp

    /** The end padding of rows, the same as the icon column's start. */
    val END = ICON_START
}
