import SwiftUI

/// The top bar's two keylines, shared by the panes, as Kotlin's `Keylines.kt` (MISC-20): the icon
/// column (the navigation button, `iconStart` to `iconStart + iconSize` from the edge) and the text
/// keyline (`textStart`, the title's start). In multi-pane windows they count from the pane's edge.
enum Metrics {
    static let iconStart: CGFloat = 16
    static let iconSize: CGFloat = 24
    static let textStart: CGFloat = 56
    /// The end padding of rows, the same as the icon column's start.
    static let end: CGFloat = 16
}
