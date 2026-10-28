import SwiftUI

/// The account setup over the panes: the step, the presets or the fields, the problem, and the
/// buttons. Which step comes next, and what is valid, are the presenter's.
struct SetupView: View {
    let sheet: SetupSheet
    let send: (UiEvent) -> Void

    var body: some View {
        Form {
            Section {
                HStack {
                    FrameButton(button: sheet.back, send: send)
                    VStack(alignment: .leading) {
                        Text(verbatim: sheet.title).font(.headline).accessibilityAddTraits(.isHeader)
                        Text(verbatim: sheet.step).font(.caption)
                    }
                }
            }
            if !sheet.presets.isEmpty {
                Section {
                    ForEach(Array(sheet.presets.enumerated()), id: \.offset) { _, preset in
                        SwiftUI.Button { send(.tap(preset.action)) } label: {
                            HStack {
                                VStack(alignment: .leading) {
                                    Text(verbatim: preset.label)
                                    Text(verbatim: preset.hint).font(.caption).foregroundStyle(.secondary)
                                }
                                Spacer()
                                if preset.selected { Image(systemName: "checkmark") }
                            }
                        }
                        .accessibilityAddTraits(preset.selected ? .isSelected : [])
                    }
                }
            }
            if !sheet.fields.isEmpty {
                Section {
                    ForEach(sheet.fields, id: \.field) { field in
                        VStack(alignment: .leading) {
                            Text(verbatim: field.label).font(.caption)
                            FrameTextField(field: field.field, value: field.value, hint: field.hint, kind: field.input, send: send)
                        }
                    }
                }
            }
            if let note = sheet.note { Section { Text(verbatim: note).font(.footnote) } }
            if let error = sheet.error { Section { Text(verbatim: error).font(.footnote).foregroundStyle(.red) } }
            Section {
                if sheet.busy { ProgressView() }
                ForEach(Array(sheet.actions.enumerated()), id: \.offset) { _, button in
                    FrameButton(button: button, send: send)
                }
            }
        }
    }
}

/// The settings page: its tabs, then groups of rows with their choices and buttons.
struct SettingsPane: View {
    let page: SettingsPage
    let send: (UiEvent) -> Void

    var body: some View {
        List {
            Section {
                ScrollView(.horizontal) {
                    HStack {
                        ForEach(page.tabs, id: \.action) { tab in
                            SwiftUI.Button { send(.tap(tab.action)) } label: { Text(verbatim: tab.label) }
                                .buttonStyle(.bordered)
                                .tint(tab.selected ? .accentColor : .secondary)
                                .accessibilityAddTraits(tab.selected ? .isSelected : [])
                        }
                    }
                }
                .listRowInsets(EdgeInsets(top: 4, leading: Metrics.textStart - Self.buttonPadding, bottom: 4, trailing: 0))
            }
            ForEach(Array(page.groups.enumerated()), id: \.offset) { _, group in
                Section {
                    ForEach(Array(group.rows.enumerated()), id: \.offset) { _, row in
                        VStack(alignment: .leading, spacing: 6) {
                            HStack {
                                Text(verbatim: row.label)
                                Spacer()
                                Text(verbatim: row.value).foregroundStyle(.secondary)
                            }
                            if let hint = row.hint { Text(verbatim: hint).font(.caption) }
                            if !row.choices.isEmpty {
                                ScrollView(.horizontal) {
                                    HStack {
                                        ForEach(row.choices, id: \.action) { choice in
                                            SwiftUI.Button { send(.tap(choice.action)) } label: { Text(verbatim: choice.label) }
                                                .buttonStyle(.bordered)
                                                .tint(choice.selected ? .accentColor : .secondary)
                                                .accessibilityAddTraits(choice.selected ? .isSelected : [])
                                        }
                                    }
                                }
                            }
                            if !row.actions.isEmpty {
                                HStack {
                                    ForEach(Array(row.actions.enumerated()), id: \.offset) { _, button in
                                        FrameButton(button: button, send: send).buttonStyle(.bordered)
                                    }
                                }
                            }
                        }
                        .listRowInsets(Self.rowInsets)
                    }
                } header: {
                    Text(verbatim: group.title).listRowInsets(Self.rowInsets)
                } footer: {
                    if let hint = group.hint { Text(verbatim: hint).listRowInsets(Self.rowInsets) }
                }
            }
        }
        .listStyle(.plain)
    }

    /// The tabs, captions and rows start on the text keyline (MISC-24).
    private static let rowInsets = EdgeInsets(top: 8, leading: Metrics.textStart, bottom: 8, trailing: Metrics.end)
    /// The label's inset inside a bordered button.
    private static let buttonPadding: CGFloat = 12
}
