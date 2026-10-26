import SwiftUI

/// The command palette over the panes, as Kotlin's `PaletteScreen.kt`: the query field (its text
/// goes back as `text` on `query`), the step's question, and the rows under their captions.
/// Ranking, steps and what a row does are the presenter's.
struct PaletteView: View {
    let sheet: PaletteSheet
    let send: (UiEvent) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                FrameButton(button: sheet.close, send: send)
                FrameTextField(field: .query, value: sheet.query, hint: sheet.hint, kind: nil, send: send)
                    .textFieldStyle(.roundedBorder)
            }
            .padding(12)
            if let prompt = sheet.prompt { Text(verbatim: prompt).font(.subheadline).padding(.horizontal, 16) }
            List {
                ForEach(Array(sheet.sections.enumerated()), id: \.offset) { _, section in
                    Section {
                        ForEach(section.rows, id: \.action) { row in
                            SwiftUI.Button { send(.tap(row.action)) } label: {
                                HStack {
                                    VStack(alignment: .leading) {
                                        Text(verbatim: row.title).lineLimit(1)
                                        if !row.detail.isEmpty {
                                            Text(verbatim: row.detail).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                                        }
                                    }
                                    Spacer()
                                    if let keys = row.keys { Text(verbatim: keys).font(.caption.monospaced()) }
                                }
                            }
                        }
                    } header: {
                        if !section.title.isEmpty { Text(verbatim: section.title) }
                    }
                }
            }
            .listStyle(.plain)
        }
    }
}

/// A text field the presenter owns: what the user types goes back as `text`; the frame's value
/// replaces the field's only when its `version` moves (a reload, not an echo of the typing).
struct FrameTextField: View {
    let field: Field
    let value: TextValue
    let hint: String
    let kind: InputKind?
    let send: (UiEvent) -> Void
    @State private var text = ""
    @State private var version: Int64 = -1
    @FocusState private var focused: Bool

    var body: some View {
        Group {
            if kind == .password {
                SecureField(hint, text: $text).textContentType(.password)
            } else {
                TextField(hint, text: $text)
                    .textContentType(contentType)
                    .keyboardType(kind == .email ? .emailAddress : kind == .server ? .URL : .default)
            }
        }
        .textInputAutocapitalization(kind == .name ? .words : .never)
        .autocorrectionDisabled(kind != .name)
        .focused($focused)
        .onAppear { load(); if field == .query || kind != nil { focused = true } }
        .onChange(of: value.version) { _, _ in load() }
        .onChange(of: text) { _, text in if text != value.text { send(.text(field, text)) } }
        .onChange(of: focused) { _, focused in if !focused { send(.blur(field)) } }
    }

    private var contentType: UITextContentType? {
        switch kind {
        case .email: .emailAddress
        case .name: .name
        case .server: .URL
        case .password: .password
        case nil: nil
        }
    }

    private func load() {
        guard value.version != version else { return }
        version = value.version
        text = value.text
    }
}
