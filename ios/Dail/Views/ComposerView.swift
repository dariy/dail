import SwiftUI

/// The composer pane (MISC-30), as Kotlin's `ComposerScreen.kt`: the title, the save state and the
/// actions, the problems, *From*, the recipient fields with their chips and suggestions, the
/// subject, the body, the attachments and the folded quote. Swift holds no rules: it shows the
/// frame's `TextValue`s, sends typed text as `.text` and leaving a field as `.blur`, and taps the
/// frame's actions. Writing back, addresses and suggestions are the presenter's
/// (`app_mobile::composer`). The pane scrolls, so the header fields can leave the screen and the
/// body get it; SwiftUI keeps the focused field above the keyboard.
struct ComposerPane: View {
    let composer: Composer
    let send: (UiEvent) -> Void

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 12) {
                    Text(verbatim: composer.title).font(.title3).lineLimit(1)
                    Spacer()
                    Text(verbatim: composer.status).font(.caption).foregroundStyle(.secondary)
                    ForEach(Array(composer.actions.enumerated()), id: \.offset) { _, button in
                        FrameButton(button: button, send: send)
                    }
                }
                ForEach(Array(composer.problems.enumerated()), id: \.offset) { _, problem in
                    Text(verbatim: problem).font(.footnote).foregroundStyle(.red)
                }
                FromRow(composer: composer, send: send)
                ForEach(Array(composer.recipients.enumerated()), id: \.offset) { _, recipients in
                    RecipientField(recipients: recipients, editable: composer.editable, send: send)
                }
                ComposerField(
                    field: .subject, value: composer.subject, label: tr("Subject"),
                    editable: composer.editable, multiline: false, send: send
                )
                Divider()
                ComposerField(
                    field: .body, value: composer.body, label: tr("Message"),
                    editable: composer.editable, multiline: true, send: send
                )
                OutgoingFiles(files: composer.attachments, send: send)
                if let quote = composer.quote {
                    Divider()
                    SwiftUI.Button { send(.tap(quote.toggle)) } label: {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(verbatim: quote.label).font(.subheadline.weight(.medium))
                            ForEach(Array(quote.header.enumerated()), id: \.offset) { _, line in
                                Text(verbatim: line).font(.footnote).foregroundStyle(.secondary)
                            }
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.horizontal, Metrics.iconStart)
            .padding(.vertical, 12)
        }
        .scrollDismissesKeyboard(.interactively)
        // A new draft starts with fresh fields.
        .id(composer.draft)
        .accessibilityIdentifier("composer")
    }
}

/// *From:* the sending address; a menu of the other connected addresses when there are some.
private struct FromRow: View {
    let composer: Composer
    let send: (UiEvent) -> Void

    var body: some View {
        let label = tr("From: {from}", ["from": composer.from])
        if composer.fromChoices.isEmpty {
            Text(verbatim: label).font(.subheadline).foregroundStyle(.secondary)
        } else {
            Menu {
                ForEach(Array(composer.fromChoices.enumerated()), id: \.offset) { _, choice in
                    SwiftUI.Button { send(.tap(choice.action)) } label: {
                        if choice.selected {
                            Label { Text(verbatim: choice.label) } icon: { Image(systemName: "checkmark") }
                        } else {
                            Text(verbatim: choice.label)
                        }
                    }
                }
            } label: {
                HStack(spacing: 4) {
                    Text(verbatim: label)
                    Image(systemName: "chevron.down").font(.caption)
                }
                .font(.subheadline)
            }
        }
    }
}

/// A recipient field: its chips (a tap removes one), the input, and the suggestions for it.
private struct RecipientField: View {
    let recipients: Recipients
    let editable: Bool
    let send: (UiEvent) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            if !recipients.chips.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 6) {
                        ForEach(Array(recipients.chips.enumerated()), id: \.offset) { _, chip in
                            SwiftUI.Button { send(.tap(chip.remove)) } label: {
                                HStack(spacing: 4) {
                                    Text(verbatim: chip.label)
                                    Image(systemName: "xmark").font(.caption2)
                                }
                                .padding(.horizontal, 10).padding(.vertical, 4)
                                .background(Color.secondary.opacity(0.15), in: Capsule())
                            }
                            .buttonStyle(.plain)
                            .disabled(!editable)
                            .accessibilityLabel(tr("Remove {name}", ["name": chip.label]))
                        }
                    }
                }
            }
            ComposerField(
                field: recipients.field, value: recipients.input, label: recipients.label,
                editable: editable, multiline: false, send: send
            )
            if !recipients.suggestions.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 6) {
                        ForEach(Array(recipients.suggestions.enumerated()), id: \.offset) { _, suggestion in
                            SwiftUI.Button { send(.tap(suggestion.action)) } label: {
                                Text(verbatim: suggestion.label)
                                    .padding(.horizontal, 10).padding(.vertical, 4)
                                    .overlay(Capsule().stroke(Color.secondary.opacity(0.4)))
                            }
                            .buttonStyle(.plain)
                        }
                    }
                }
            }
        }
    }
}

/// The attached files: name and size, *checking…* or why it cannot be sent; a tap removes one.
private struct OutgoingFiles: View {
    let files: [OutgoingFile]
    let send: (UiEvent) -> Void

    var body: some View {
        if !files.isEmpty {
            VStack(alignment: .leading, spacing: 4) {
                ForEach(Array(files.enumerated()), id: \.offset) { _, file in
                    HStack(spacing: 8) {
                        Image(systemName: "paperclip")
                        VStack(alignment: .leading, spacing: 0) {
                            Text(verbatim: file.name).lineLimit(1)
                            Text(verbatim: file.detail).font(.caption)
                                .foregroundStyle(file.failed ? Color.red : Color.secondary)
                        }
                        Spacer()
                        if let remove = file.remove {
                            SwiftUI.Button { send(.tap(remove)) } label: { Image(systemName: "xmark") }
                                .accessibilityLabel(tr("Remove {name}", ["name": file.name]))
                        }
                    }
                }
            }
        }
    }
}

/// One text field of the composer. It loads the frame's text only when its `version` changes, so
/// the presenter's own write-back (an auto-save) never moves the caret or the keyboard (MISC-31's
/// rule, kept on iOS).
private struct ComposerField: View {
    let field: Field
    let value: TextValue
    let label: String
    let editable: Bool
    let multiline: Bool
    let send: (UiEvent) -> Void
    @State private var text = ""
    @State private var version: Int64 = -1
    @FocusState private var focused: Bool

    var body: some View {
        Group {
            if multiline {
                TextField(label, text: $text, axis: .vertical)
                    .lineLimit(8...)
            } else {
                TextField(label, text: $text)
                    .keyboardType(field == .subject ? .default : .emailAddress)
                    .textContentType(field == .subject ? nil : .emailAddress)
                    .textInputAutocapitalization(field == .subject ? .sentences : .never)
                    .autocorrectionDisabled(field != .subject)
            }
        }
        .disabled(!editable)
        .focused($focused)
        .padding(.vertical, 6)
        .onAppear(perform: load)
        .onChange(of: value.version) { _, _ in load() }
        .onChange(of: text) { _, text in if text != value.text { send(.text(field, text)) } }
        .onChange(of: focused) { _, focused in if !focused { send(.blur(field)) } }
    }

    private func load() {
        guard value.version != version else { return }
        version = value.version
        text = value.text
    }
}
