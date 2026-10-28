import SwiftUI

/// The frame theme's colour roles (`#RRGGBBAA`), with the system's colours where a role is missing.
struct Palette {
    let content: [String: Color]
    let chrome: [String: Color]
    let tags: [Color]

    init(theme: FrameTheme?) {
        content = (theme?.content ?? [:]).compactMapValues(Color.init(rgba:))
        chrome = (theme?.chrome ?? [:]).compactMapValues(Color.init(rgba:))
        tags = (theme?.tags ?? []).compactMap(Color.init(rgba:))
    }

    /// An account's colour (`Row.tag`), as Kotlin's `Chrome.tag`.
    func tag(_ index: Int) -> Color { tags.indices.contains(index) ? tags[index] : .clear }

    func role(_ name: String, _ fallback: Color) -> Color { content[name] ?? fallback }
    func chrome(_ name: String, _ fallback: Color) -> Color { chrome[name] ?? fallback }
}

extension Color {
    /// `#RRGGBB` or `#RRGGBBAA`.
    init?(rgba: String) {
        guard rgba.hasPrefix("#"), let value = UInt64(rgba.dropFirst(), radix: 16) else { return nil }
        let digits = rgba.count - 1
        guard digits == 6 || digits == 8 else { return nil }
        let rgba = digits == 6 ? value << 8 | 0xFF : value
        self.init(
            .sRGB,
            red: Double(rgba >> 24 & 0xFF) / 255, green: Double(rgba >> 16 & 0xFF) / 255,
            blue: Double(rgba >> 8 & 0xFF) / 255, opacity: Double(rgba & 0xFF) / 255)
    }
}

/// The frame's icon names (Material Symbols, as on Android) as SF Symbols.
enum Icons {
    static let symbols: [String: String] = [
        "add": "plus", "all_inbox": "tray.2", "archive": "archivebox", "attach_file": "paperclip",
        "arrow_back": "arrow.backward", "open_in_full": "arrow.up.left.and.arrow.down.right",
        "close_fullscreen": "arrow.down.right.and.arrow.up.left", "check": "checkmark", "check_box": "checkmark.square",
        "check_box_outline_blank": "square", "chevron_left": "chevron.left", "chevron_right": "chevron.right",
        "close": "xmark", "cloud_off": "icloud.slash", "content_copy": "doc.on.doc", "contrast": "circle.lefthalf.filled",
        "dark_mode": "moon", "delete": "trash", "description": "doc.text", "dock_to_left": "sidebar.left",
        "download": "arrow.down.circle", "draft": "doc", "drive_file_move": "folder.badge.plus", "edit": "square.and.pencil",
        "error": "exclamationmark.circle", "expand_less": "chevron.up", "expand_more": "chevron.down",
        "folder": "folder", "forward": "arrowshape.turn.up.right", "image": "photo", "inbox": "tray", "info": "info.circle",
        "keyboard": "keyboard", "light_mode": "sun.max", "link": "link", "mail": "envelope",
        "mark_email_read": "envelope.open", "mark_email_unread": "envelope.badge", "menu": "line.3.horizontal",
        "more_horiz": "ellipsis", "open_in_new": "arrow.up.right.square", "person": "person",
        "picture_as_pdf": "doc.richtext", "push_pin": "pin", "reply": "arrowshape.turn.up.left", "reply_all": "arrowshape.turn.up.left.2",
        "report": "exclamationmark.octagon", "schedule": "clock", "search": "magnifyingglass", "send": "paperplane",
        "settings": "gearshape", "star": "star", "sync": "arrow.triangle.2.circlepath", "undo": "arrow.uturn.backward",
        "unfold_less": "arrow.down.right.and.arrow.up.left", "unfold_more": "arrow.up.left.and.arrow.down.right",
        "verified_user": "checkmark.shield", "visibility": "eye", "visibility_off": "eye.slash", "warning": "exclamationmark.triangle",
    ]

    static func symbol(_ name: String?) -> String? { name.flatMap { symbols[$0] } }
}

/// A frame button: its icon if it has one (the label for VoiceOver), its label otherwise.
struct FrameButton: View {
    let button: ActionButton
    let send: (UiEvent) -> Void

    var body: some View {
        SwiftUI.Button { send(.tap(button.action)) } label: {
            if let symbol = Icons.symbol(button.icon) {
                Image(systemName: symbol).accessibilityLabel(button.label)
            } else {
                Text(button.label)
            }
        }
    }
}
