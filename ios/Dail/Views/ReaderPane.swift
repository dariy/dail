import SwiftUI

/// The open thread, as Kotlin's `ReaderScreen.kt`: the actions, then each message, collapsed or with
/// its body. Bodies are `DocBlock`s Rust made from the text or the cleaned HTML; Swift draws them as
/// text and never loads anything: remote images stay off, links go through the link sheet.
struct ReaderPane: View {
    let reader: Reader
    let send: (UiEvent) -> Void

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                Text(verbatim: reader.subject).font(.title2).accessibilityAddTraits(.isHeader)
                HStack(spacing: 20) {
                    ForEach(Array(reader.actions.enumerated()), id: \.offset) { _, button in
                        FrameButton(button: button, send: send)
                    }
                    Spacer()
                    if let previous = reader.previous { FrameButton(button: previous, send: send) }
                    if let position = reader.position { Text(verbatim: position).font(.caption) }
                    if let next = reader.next { FrameButton(button: next, send: send) }
                }
                ForEach(Array(reader.messages.enumerated()), id: \.offset) { _, message in
                    MessageView(message: message, send: send)
                    Divider()
                }
            }
            .padding()
        }
        // `dail-action:<id>` is how a link span reaches its action; nothing else is opened.
        .environment(\.openURL, OpenURLAction { url in
            if url.scheme == "dail-action", let id = Int(url.absoluteString.dropFirst("dail-action:".count)) {
                send(.tap(id))
            }
            return .handled
        })
    }
}

struct MessageView: View {
    let message: ReaderMessage
    let send: (UiEvent) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            SwiftUI.Button { send(.tap(message.toggle)) } label: {
                VStack(alignment: .leading, spacing: 2) {
                    HStack {
                        Text(verbatim: message.from).fontWeight(.semibold).lineLimit(1)
                        Spacer()
                        Text(verbatim: message.date).font(.caption)
                    }
                    Text(verbatim: message.expanded ? message.to : message.snippet).font(.footnote).lineLimit(1)
                        .foregroundStyle(.secondary)
                }
            }
            .buttonStyle(.plain)
            .accessibilityAddTraits(message.expanded ? .isSelected : [])
            if message.expanded {
                if let status = message.status { Text(verbatim: status).font(.footnote).italic() }
                if let images = message.images {
                    HStack {
                        Text(verbatim: images.text).font(.footnote)
                        Spacer()
                        FrameButton(button: images.toggle, send: send)
                    }
                }
                ForEach(Array((message.body ?? []).enumerated()), id: \.offset) { _, block in
                    BlockView(block: block, send: send)
                }
                ForEach(Array(message.attachments.enumerated()), id: \.offset) { _, attachment in
                    HStack {
                        Image(systemName: "paperclip")
                        SwiftUI.Button { send(.tap(attachment.open)) } label: {
                            Text(verbatim: "\(attachment.name) · \(attachment.size)")
                        }
                        Spacer()
                        SwiftUI.Button { send(.tap(attachment.save)) } label: { Image(systemName: "arrow.down.circle") }
                            .accessibilityLabel("Save \(attachment.name)")
                    }
                }
                if let saveAll = message.saveAll { FrameButton(button: saveAll, send: send) }
            }
        }
    }
}

/// One body block. Text is untrusted: it is shown verbatim, never as Markdown.
struct BlockView: View {
    let block: DocBlock
    let send: (UiEvent) -> Void

    var body: some View {
        switch block {
        case let .paragraph(spans):
            Text(Self.text(spans)).textSelection(.enabled)
        case let .heading(level, spans):
            Text(Self.text(spans)).font(level <= 1 ? .title3 : .headline).accessibilityAddTraits(.isHeader)
        case let .list(ordered, start, items):
            VStack(alignment: .leading, spacing: 4) {
                ForEach(Array(items.enumerated()), id: \.offset) { index, blocks in
                    HStack(alignment: .firstTextBaseline) {
                        Text(verbatim: ordered ? "\(start + index)." : "•")
                        VStack(alignment: .leading) {
                            ForEach(Array(blocks.enumerated()), id: \.offset) { _, block in BlockView(block: block, send: send) }
                        }
                    }
                }
            }
        case let .quote(blocks):
            VStack(alignment: .leading) {
                ForEach(Array(blocks.enumerated()), id: \.offset) { _, block in BlockView(block: block, send: send) }
            }
            .padding(.leading, 10)
            .overlay(alignment: .leading) { Rectangle().frame(width: 3).foregroundStyle(.secondary) }
        case let .preformatted(lines):
            ScrollView(.horizontal) {
                Text(verbatim: lines.joined(separator: "\n")).font(.footnote.monospaced()).textSelection(.enabled)
            }
        case let .table(rows):
            Grid(alignment: .leading) {
                ForEach(Array(rows.enumerated()), id: \.offset) { _, cells in
                    GridRow {
                        ForEach(Array(cells.enumerated()), id: \.offset) { _, cell in
                            Text(Self.text(cell.spans)).fontWeight(cell.header == true ? .semibold : .regular)
                        }
                    }
                }
            }
        case .rule:
            Divider()
        case let .fold(label, expanded, toggle, blocks):
            VStack(alignment: .leading) {
                SwiftUI.Button { send(.tap(toggle)) } label: {
                    Label(label, systemImage: expanded ? "chevron.up" : "ellipsis")
                }
                if expanded {
                    ForEach(Array(blocks.enumerated()), id: \.offset) { _, block in BlockView(block: block, send: send) }
                }
            }
        }
    }

    /// The spans as one attributed string. A link span points at its action, never at its URL; an
    /// inline image is a placeholder (images are off until the user shows them, I3 scope).
    static func text(_ spans: [DocSpan]) -> AttributedString {
        spans.reduce(into: AttributedString()) { text, span in
            var piece = AttributedString(span.image == nil ? span.text : "[\(span.text.isEmpty ? "image" : span.text)]")
            var intent: InlinePresentationIntent = []
            if span.bold == true { intent.insert(.stronglyEmphasized) }
            if span.italic == true { intent.insert(.emphasized) }
            if span.code == true { intent.insert(.code) }
            if span.strike == true { intent.insert(.strikethrough) }
            if !intent.isEmpty { piece.inlinePresentationIntent = intent }
            if span.hit == true { piece.backgroundColor = .yellow.opacity(0.4) }
            if let link = span.link { piece.link = URL(string: "dail-action:\(link)") }
            text += piece
        }
    }
}
