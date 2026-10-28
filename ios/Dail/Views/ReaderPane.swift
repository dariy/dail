import SwiftUI

/// The open thread, as Kotlin's `ReaderScreen.kt`: the actions, then each message, collapsed or with
/// its body. Bodies are `DocBlock`s Rust made from the text or the cleaned HTML; Swift draws them as
/// text and never loads anything: remote images stay off, links go through the link sheet.
struct ReaderPane: View {
    let reader: Reader
    let send: (UiEvent) -> Void
    /// Whether a horizontal swipe turns to the next or previous conversation (MISC-5). A page's
    /// swipe right goes back instead (MISC-18).
    var swipes = true

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                HStack(alignment: .firstTextBaseline) {
                    Text(verbatim: reader.subject).font(.title2).accessibilityAddTraits(.isHeader)
                    Spacer()
                    // MISC-7: the × closes the message; MISC-14: maximize / restore beside it.
                    if let maximize = reader.maximize { FrameButton(button: maximize, send: send) }
                    FrameButton(button: reader.close, send: send)
                }
                HStack(spacing: 20) {
                    ForEach(Array(reader.actions.enumerated()), id: \.offset) { _, button in
                        FrameButton(button: button, send: send)
                    }
                }
                ForEach(Array(reader.messages.enumerated()), id: \.offset) { _, message in
                    MessageView(message: message, send: send)
                    Divider()
                }
            }
            .padding()
        }
        // MISC-5: beside the list a horizontal swipe goes to the next (left) or previous (right)
        // conversation; VoiceOver gets the same as named actions.
        .simultaneousGesture(DragGesture(minimumDistance: 30).onEnded { drag in
            let dx = drag.translation.width
            guard abs(dx) > 72, abs(dx) > abs(drag.translation.height) else { return }
            if let target = dx < 0 ? reader.next : reader.previous { send(.tap(target.action)) }
        }, including: swipes ? .all : .subviews)
        // MISC-6: a pinch steps the pane layout, one step per gesture.
        .simultaneousGesture(MagnifyGesture().onEnded { pinch in
            let scale = pinch.magnification
            if scale > 1.15 || scale < 1 / 1.15 { send(.pinch(zoomIn: scale > 1)) }
        })
        .accessibilityActions {
            if let previous = reader.previous {
                SwiftUI.Button(previous.label) { send(.tap(previous.action)) }
            }
            if let next = reader.next {
                SwiftUI.Button(next.label) { send(.tap(next.action)) }
            }
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

/// The open message on a compact window: a page over the list and the top bar, below the status
/// bar, as Kotlin's `ReaderPage.kt` (MISC-18). It slides in from the right (the frame view's
/// transition). A horizontal drag follows the finger; the dominant axis decides against the body's
/// scroll. Released past a third of the width, or flung, it slides away and sends the reader's
/// close (`close_thread`); otherwise it springs back. While a screen that takes the panes' place is
/// up (`covered`) it waits unseen, in place. VoiceOver's escape gesture closes it. With Reduce
/// Motion it appears and goes without the slide.
struct ReaderPage: View {
    let reader: Reader
    let covered: Bool
    let send: (UiEvent) -> Void
    @Environment(\.accessibilityReduceMotion) private var still
    @State private var offset: CGFloat = 0
    /// Whether the drag in progress moves the page (horizontal) or not (vertical), once decided.
    @State private var sliding: Bool?

    var body: some View {
        GeometryReader { geometry in
            let width = max(geometry.size.width, 1)
            ZStack {
                Color.black.opacity(0.32 * (1 - offset / width)).ignoresSafeArea().allowsHitTesting(false)
                ReaderPane(reader: reader, send: send, swipes: false)
                    .background(Color(uiColor: .systemBackground))
                    .shadow(radius: offset > 0 ? 8 : 0)
                    .offset(x: offset)
                    .simultaneousGesture(swipe(width))
            }
        }
        .opacity(covered ? 0 : 1)
        .allowsHitTesting(!covered)
        .accessibilityHidden(covered)
        .accessibilityAction(.escape) { send(.tap(reader.close.action)) }
    }

    private func swipe(_ width: CGFloat) -> some Gesture {
        // Global coordinates: the page moves under the finger.
        DragGesture(minimumDistance: 12, coordinateSpace: .global)
            .onChanged { drag in
                let t = drag.translation
                if sliding == nil { sliding = abs(t.width) > abs(t.height) }
                guard sliding == true else { return }
                offset = max(0, t.width)
            }
            .onEnded { drag in
                defer { sliding = nil }
                guard sliding == true, offset > 0 else { return }
                let flung = drag.predictedEndTranslation.width - drag.translation.width > 200
                if offset > width / 3 || flung {
                    if still {
                        send(.tap(reader.close.action))
                    } else {
                        withAnimation(.easeIn(duration: 0.2)) { offset = width }
                        DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) { send(.tap(reader.close.action)) }
                    }
                    // A close the presenter ignored (from a frame already replaced) must not leave
                    // the message open off screen: it comes back.
                    DispatchQueue.main.asyncAfter(deadline: .now() + 1.2) {
                        guard offset > 0 else { return }
                        if still { offset = 0 } else { withAnimation(.spring) { offset = 0 } }
                    }
                } else if still {
                    offset = 0
                } else {
                    withAnimation(.spring) { offset = 0 }
                }
            }
    }
}
