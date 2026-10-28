import SwiftUI

/// The open thread, as Kotlin's `ReaderScreen.kt`: the actions, then each message, collapsed or with
/// its body. Bodies are `DocBlock`s Rust made from the text or the cleaned HTML; Swift draws them as
/// text and never loads anything: remote images stay off, links go through the link sheet.
///
/// Pulling past the end of the body shows the next conversation's peek, and past its top the
/// previous one's (MISC-18). Released beyond `turn`, the body slides out, the peek slides in, and the
/// conversation's frame replaces it; short of it, the bounce takes it back. The finger's release
/// decides, so a fling's momentum never turns. With Reduce Motion the conversations cross-fade.
/// VoiceOver gets both conversations as named actions.
struct ReaderPane: View {
    let reader: Reader
    let send: (UiEvent) -> Void
    @Environment(\.accessibilityReduceMotion) private var still
    /// How far the body is pulled past its top (> 0) or its end (< 0), in points.
    @State private var pull: CGFloat = 0
    /// The conversation a released pull turns to, while its peek slides in from `way` (-1 below,
    /// 1 above), and the turn's number.
    @State private var turning: Neighbour?
    @State private var way: CGFloat = 0
    @State private var turns = 0
    /// The body's slide while it turns.
    @State private var shift: CGFloat = 0

    var body: some View {
        GeometryReader { geometry in
            let height = geometry.size.height
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
                .background(GeometryReader { content in
                    Color.clear.preference(
                        key: ReaderPullKey.self, value: Self.pull(content.frame(in: .named("reader")), height))
                })
            }
            .coordinateSpace(name: "reader")
            .scrollBounceBehavior(.always, axes: .vertical)
            .onPreferenceChange(ReaderPullKey.self) { value in if turning == nil { pull = value } }
            // A new conversation starts at its top.
            .id(reader.thread)
            .transition(.opacity)
            .offset(y: shift)
            .overlay(alignment: pull < 0 ? .bottom : .top) {
                // The gap a pull opens shows who is there.
                if turning == nil, let peek = pull < 0 ? reader.next : (pull > 0 ? reader.previous : nil) {
                    PeekView(peek: peek, ready: abs(pull) > Self.turn)
                        .frame(maxWidth: .infinity)
                        .frame(height: abs(pull))
                        .clipped()
                }
            }
            .overlay {
                if let turning {
                    PeekView(peek: turning, ready: false)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                        .background(Color(uiColor: .systemBackground))
                        .offset(y: shift - way * height)
                }
            }
            .clipped()
            .simultaneousGesture(DragGesture(minimumDistance: 10).onEnded { _ in release(height) })
        }
        .animation(still ? .easeInOut(duration: 0.2) : nil, value: reader.thread)
        .onChange(of: reader.thread) {
            turning = nil
            shift = 0
            pull = 0
        }
        // MISC-6: a pinch steps the pane layout, one step per gesture.
        .simultaneousGesture(MagnifyGesture().onEnded { pinch in
            let scale = pinch.magnification
            if scale > 1.15 || scale < 1 / 1.15 { send(.pinch(zoomIn: scale > 1)) }
        })
        .accessibilityActions {
            if let previous = reader.previous {
                SwiftUI.Button(previous.open.label) { send(.tap(previous.open.action)) }
            }
            if let next = reader.next {
                SwiftUI.Button(next.open.label) { send(.tap(next.open.action)) }
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

    /// When the finger lifts: turns to the neighbour a pull past `turn` shows.
    private func release(_ height: CGFloat) {
        guard turning == nil, abs(pull) > Self.turn,
              let target = pull < 0 ? reader.next : reader.previous else { return }
        if still {
            send(.tap(target.open.action))
            return
        }
        way = pull < 0 ? -1 : 1
        turning = target
        turns += 1
        let mine = turns
        withAnimation(.easeIn(duration: 0.25)) { shift = way * height }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.25) { send(.tap(target.open.action)) }
        // A turn the presenter ignored (from a frame already replaced) must not leave the body off
        // screen: it comes back.
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.25) {
            guard turning != nil, turns == mine else { return }
            turning = nil
            withAnimation(.spring) { shift = 0 }
        }
    }

    /// How far the body (`frame`, in the scroll view's space) is pulled past its top (> 0) or its
    /// end (< 0) in a viewport `height` tall; 0 in between.
    static func pull(_ frame: CGRect, _ height: CGFloat) -> CGFloat {
        if frame.minY > 1 { return frame.minY }
        let past = min(frame.height, height) - frame.maxY
        return past > 1 ? -past : 0
    }

    /// The pull that turns the conversation.
    static let turn: CGFloat = 80
}

/// A neighbour's peek (MISC-18): which way, then its subject, senders and date as its list row
/// shows them.
private struct PeekView: View {
    let peek: Neighbour
    let ready: Bool

    var body: some View {
        VStack(spacing: 4) {
            Label(ready ? "Release to open" : peek.open.label,
                  systemImage: Icons.symbol(peek.open.icon) ?? "chevron.up.chevron.down")
                .font(.footnote).foregroundStyle(.secondary)
            Text(verbatim: peek.subject).font(.headline).lineLimit(2).multilineTextAlignment(.center)
            Text(verbatim: "\(peek.sender) · \(peek.date)").font(.footnote).lineLimit(1)
        }
        .padding(.horizontal)
        .accessibilityHidden(true)
    }
}

/// How far the reader's body is pulled past an end (`ReaderPane.pull`).
private struct ReaderPullKey: PreferenceKey {
    static let defaultValue: CGFloat = 0
    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) { value = nextValue() }
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
                ReaderPane(reader: reader, send: send)
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
