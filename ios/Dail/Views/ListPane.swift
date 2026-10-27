import SwiftUI

/// The thread list, as Kotlin's `ListScreen.kt`: `total` rows, of which the frame carries a window
/// (`offset`, `rows`); the others are placeholders until the window moves there. Swift reports
/// what is visible (`scrolled`) and the presenter sends the rows around it.
struct ListPane: View {
    let list: ThreadList
    let palette: Palette
    let send: (UiEvent) -> Void
    @State private var visible: Set<Int> = []
    @State private var reported: (first: Int, count: Int)?

    var body: some View {
        Group {
            if list.total == 0 {
                ScrollView {
                    VStack(spacing: 8) {
                        Image(systemName: "tray").font(.largeTitle)
                        if let text = list.server ?? list.empty { Text(verbatim: text).multilineTextAlignment(.center) }
                    }
                    .padding(.top, 96).frame(maxWidth: .infinity)
                }
            } else {
                List {
                    if let server = list.server { Text(verbatim: server).font(.caption) }
                    ForEach(0..<list.total, id: \.self) { index in
                        row(index)
                            .onAppear { visible.insert(index); report() }
                            .onDisappear { visible.remove(index); report() }
                    }
                }
                .listStyle(.plain)
            }
        }
        .refreshable { send(.refresh) }
    }

    @ViewBuilder private func row(_ index: Int) -> some View {
        let position = index - list.offset
        if position >= 0, position < list.rows.count {
            let row = list.rows[position]
            ThreadRowView(row: row, density: list.density)
                .contentShape(Rectangle())
                .onTapGesture { send(.tap(row.action)) }
                .onLongPressGesture { send(.longPress(row.longPress)) }
                .swipeActions(edge: .leading) {
                    SwiftUI.Button { send(.swipe(row.swipeStart)) } label: {
                        Image(systemName: "archivebox")
                    }
                }
                .swipeActions(edge: .trailing) {
                    SwiftUI.Button(role: .destructive) { send(.swipe(row.swipeEnd)) } label: { Image(systemName: "trash") }
                        .accessibilityLabel("Trash")
                }
                .listRowBackground(
                    row.checked
                        ? palette.role("secondary_container", Color.accentColor.opacity(0.25))
                        : row.selected ? palette.role("primary_container", Color.accentColor.opacity(0.12)) : nil)
                .accessibilityElement(children: .ignore)
                .accessibilityLabel(row.a11y)
                .accessibilityAddTraits(.isButton)
                .accessibilityAction(named: "Select") { send(.longPress(row.longPress)) }
        } else {
            Text(" ").frame(height: list.density == .compact ? 24 : 56).redacted(reason: .placeholder)
        }
    }

    /// Sends `scrolled` whenever the first visible row or the number of visible rows changes.
    private func report() {
        guard let first = visible.min() else { return }
        let count = visible.count
        if reported?.first == first, reported?.count == count { return }
        reported = (first, count)
        send(.scrolled(first: first, count: count))
    }
}

/// One thread, in the frame's density: one line, two, or three with the snippet.
struct ThreadRowView: View {
    let row: Row
    let density: Density

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            Circle().fill(row.unread ? Color.accentColor : .clear).frame(width: 8, height: 8).padding(.top, 6)
            VStack(alignment: .leading, spacing: 2) {
                HStack {
                    Text(verbatim: row.sender).lineLimit(1)
                    if row.messages > 1 { Text("\(row.messages)").font(.caption).foregroundStyle(.secondary) }
                    Spacer()
                    if row.attachments { Image(systemName: "paperclip").font(.caption) }
                    if row.starred { Image(systemName: "star.fill").font(.caption).foregroundStyle(.yellow) }
                    if row.pinned == true { Image(systemName: "pin.fill").font(.caption).accessibilityLabel("Pinned") }
                    Text(verbatim: row.date).font(.caption).foregroundStyle(.secondary)
                }
                .fontWeight(row.unread ? .semibold : .regular)
                if density != .compact {
                    marked(row.marks?.subject, row.subject).lineLimit(1)
                        .fontWeight(row.unread ? .semibold : .regular)
                }
                if density == .preview {
                    marked(row.marks?.snippet, row.snippet).font(.footnote).foregroundStyle(.secondary).lineLimit(2)
                }
            }
        }
        .padding(.vertical, density == .compact ? 2 : 6)
    }

    /// The text with the search words in bold.
    private func marked(_ pieces: [Piece]?, _ text: String) -> Text {
        guard let pieces else { return Text(verbatim: text) }
        return pieces.reduce(Text(verbatim: "")) { line, piece in
            line + (piece.hit == true ? Text(verbatim: piece.text).bold() : Text(verbatim: piece.text))
        }
    }
}
