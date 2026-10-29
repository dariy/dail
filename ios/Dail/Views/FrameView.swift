import SwiftUI

/// Draws a frame as it is, as Kotlin's `DailScreen.kt`: the top bar (or the bulk bar), the banner,
/// the panes the layout names, the snackbar and the floating button, under the modal drawer of
/// compact windows. Sheets a later wave draws (about, preview, composer) show their title for now.
struct FrameView: View {
    let frame: Frame
    let palette: Palette
    let send: (UiEvent) -> Void
    @Environment(\.accessibilityReduceMotion) private var still

    var body: some View {
        VStack(spacing: 0) {
            TopBarView(frame: frame, palette: palette, send: send)
            if let banner = frame.banner { BannerView(banner: banner, send: send) }
            content
        }
        .overlay(alignment: .bottomTrailing) {
            if let fab = frame.fab, frame.snackbar == nil {
                // A round icon button; VoiceOver reads its name (MISC-21).
                SwiftUI.Button { send(.tap(fab.action)) } label: {
                    Image(systemName: Icons.symbol(fab.icon) ?? "square.and.pencil")
                        .font(.title2)
                        .frame(width: 56, height: 56)
                        .background(palette.role("primary_container", Color.accentColor.opacity(0.2)), in: Circle())
                }
                .accessibilityLabel(fab.label)
                .padding(20)
            }
        }
        // Under the page the list keeps its place, hidden from VoiceOver (MISC-18).
        .accessibilityHidden(onPage)
        .overlay {
            if let reader = pageReader {
                ReaderPage(reader: reader, covered: covered, send: send)
                    .transition(still ? .identity : .move(edge: .trailing))
            }
        }
        // It slides in and out over the list; it goes at once when something else replaces it.
        .animation(still || frame.layout.panes != [.list] ? nil : .easeOut(duration: 0.25), value: pageReader != nil)
        // Over the page: the undo of what was just done there.
        .overlay(alignment: .bottom) {
            if let snackbar = frame.snackbar { SnackbarView(snackbar: snackbar, send: send) }
        }
        .sheet(isPresented: modalDrawer) {
            if let drawer = frame.drawer { DrawerPane(drawer: drawer, send: send) }
        }
        .alert(frame.link?.url ?? "", isPresented: linkShown, presenting: frame.link) { link in
            SwiftUI.Button(link.open.label) { send(.tap(link.open.action)) }
            SwiftUI.Button(link.copy.label) { send(.tap(link.copy.action)) }
            SwiftUI.Button(link.close.label, role: .cancel) { send(.tap(link.close.action)) }
        }
        .background(palette.role("surface", Color(uiColor: .systemBackground)))
        .foregroundStyle(palette.role("on_surface", Color.primary))
    }

    @ViewBuilder private var content: some View {
        if let about = frame.about {
            // Shown only while the frame has it; the top bar above carries its close (MISC-12).
            AboutView(about: about)
        } else if let result = frame.result {
            ResultPane(result: result, send: send)
        } else if let sheet = frame.setup {
            SetupView(sheet: sheet, send: send)
        } else if let sheet = frame.palette {
            PaletteView(sheet: sheet, send: send)
        } else if let preview = frame.preview {
            PreviewView(preview: preview, send: send)
        } else {
            HStack(spacing: 0) {
                ForEach(Array(frame.layout.panes.enumerated()), id: \.offset) { _, pane in
                    paneView(pane)
                    Divider()
                }
            }
        }
    }

    @ViewBuilder private func paneView(_ pane: PaneKind) -> some View {
        switch pane {
        case .drawer:
            if let drawer = frame.drawer, !drawer.modal {
                DrawerPane(drawer: drawer, send: send).frame(width: drawer.rail ? 88 : 300)
            }
        case .list:
            if let list = frame.list { ListPane(list: list, palette: palette, send: send) }
        case .reader:
            if let reader = frame.reader { ReaderPane(reader: reader, send: send) }
        case .composer:
            if let composer = frame.composer { Placeholder(title: composer.title, text: composer.status) }
        case .settings:
            if let settings = frame.settings { SettingsPane(page: settings, send: send) }
        }
    }

    /// The open message drawn as a page over the panes (compact windows, MISC-18).
    private var pageReader: Reader? { frame.layout.page == .reader ? frame.reader : nil }

    /// What takes the panes' place shows over the page, which waits under it, in place.
    private var covered: Bool {
        frame.about != nil || frame.setup != nil || frame.palette != nil || frame.result != nil || frame.preview != nil
    }

    private var onPage: Bool { pageReader != nil && !covered }

    /// The drawer of compact windows is a sheet; dismissing it is Back, as on Android.
    private var modalDrawer: Binding<Bool> {
        Binding(get: { frame.drawer?.modal == true }, set: { shown in if !shown { send(.back) } })
    }

    private var linkShown: Binding<Bool> {
        Binding(get: { frame.link != nil }, set: { shown in
            if !shown, let link = frame.link { send(.tap(link.close.action)) }
        })
    }
}

struct TopBarView: View {
    let frame: Frame
    let palette: Palette
    let send: (UiEvent) -> Void

    var body: some View {
        HStack(spacing: 16) {
            if let bulk = frame.bulk {
                FrameButton(button: bulk.close, send: send)
                Text(bulk.title).font(.headline)
                Spacer()
                ForEach(Array(bulk.actions.enumerated()), id: \.offset) { _, button in FrameButton(button: button, send: send) }
            } else {
                // The navigation button in a fixed slot, so the title starts on the text keyline (MISC-20).
                if let navigation = frame.topBar.navigation {
                    FrameButton(button: navigation, send: send)
                        .frame(width: Metrics.iconSize)
                        .padding(.trailing, Metrics.textStart - Metrics.iconStart - Metrics.iconSize - 16)
                }
                VStack(alignment: .leading, spacing: 0) {
                    Text(frame.topBar.title).font(.headline).lineLimit(1)
                        .accessibilityAddTraits(.isHeader)
                    if let subtitle = frame.topBar.subtitle { Text(subtitle).font(.caption2) }
                }
                Spacer()
                ForEach(Array(frame.topBar.actions.enumerated()), id: \.offset) { _, button in
                    FrameButton(button: button, send: send)
                }
            }
        }
        .padding(.horizontal, Metrics.iconStart).padding(.vertical, 10)
        .foregroundStyle(palette.chrome("text_strong", Color.primary))
        .background(palette.chrome("base", Color(uiColor: .secondarySystemBackground)))
    }
}

/// The first problem and its fix (`app_front::status`), or news (`info`): a new version.
struct BannerView: View {
    let banner: Banner
    let send: (UiEvent) -> Void

    var body: some View {
        HStack(spacing: 12) {
            if banner.info != true { Image(systemName: "exclamationmark.triangle") }
            VStack(alignment: .leading) {
                Text(banner.text).fontWeight(.semibold)
                if let hint = banner.hint { Text(hint).font(.footnote) }
            }
            Spacer()
            if let dismiss = banner.dismiss { FrameButton(button: dismiss, send: send) }
            if let action = banner.action { FrameButton(button: action, send: send) }
        }
        .padding(12)
        .background(
            banner.info == true ? Color.accentColor.opacity(0.12) : Color.red.opacity(0.12),
            in: RoundedRectangle(cornerRadius: 12)
        )
        .padding(8)
    }
}

/// What just happened, with its undo.
struct SnackbarView: View {
    let snackbar: Snackbar
    let send: (UiEvent) -> Void

    var body: some View {
        HStack {
            Text(snackbar.text)
            Spacer()
            if let action = snackbar.action { FrameButton(button: action, send: send).bold() }
        }
        .padding(14)
        .foregroundStyle(Color(uiColor: .systemBackground))
        .background(Color(uiColor: .label), in: RoundedRectangle(cornerRadius: 10))
        .padding(12)
        .accessibilityElement(children: .contain)
    }
}

/// A query's result under the note that it is untrusted: sections of labelled fields and rows in
/// the list's style, or the raw JSON after *Raw data* (MISC-22). All of it is plain text.
struct ResultPane: View {
    let result: ResultView
    let send: (UiEvent) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                FrameButton(button: result.close, send: send)
                Text(result.title).font(.title3)
                Spacer()
                FrameButton(button: result.raw, send: send)
            }
            .padding()
            Text(result.note).font(.footnote).padding(.horizontal)
            List {
                ForEach(Array((result.sections ?? []).enumerated()), id: \.offset) { _, section in
                    Section {
                        ForEach(Array((section.fields ?? []).enumerated()), id: \.offset) { _, field in
                            HStack(alignment: .firstTextBaseline) {
                                if !field.label.isEmpty {
                                    Text(verbatim: field.label).font(.caption).foregroundStyle(.secondary)
                                        .frame(width: 112, alignment: .leading)
                                }
                                Text(verbatim: field.value).textSelection(.enabled)
                            }
                            .listRowInsets(EdgeInsets(top: 4, leading: Metrics.textStart, bottom: 4, trailing: Metrics.end))
                        }
                        ForEach(Array((section.rows ?? []).enumerated()), id: \.offset) { _, row in
                            rowView(row)
                        }
                    } header: {
                        if let title = section.title { Text(verbatim: title) }
                    }
                }
                ForEach(Array((result.lines ?? []).enumerated()), id: \.offset) { _, line in
                    Text(verbatim: line).font(.body.monospaced()).textSelection(.enabled)
                }
            }
            .listStyle(.plain)
        }
    }

    @ViewBuilder private func rowView(_ row: ResultRow) -> some View {
        let marks = row.marks ?? []
        let unread = marks.contains("unread")
        HStack(spacing: 0) {
            Group {
                if marks.contains("failed") {
                    Image(systemName: "xmark.circle").foregroundStyle(.red).accessibilityLabel("Failed")
                } else if marks.contains("passed") {
                    Image(systemName: "checkmark.circle").foregroundStyle(.green).accessibilityLabel("Passed")
                } else if marks.contains("pending") {
                    Image(systemName: "clock").accessibilityLabel("Pending")
                } else {
                    Circle().fill(unread ? Color.accentColor : .clear).frame(width: 8, height: 8)
                }
            }
            .frame(width: Metrics.textStart)
            VStack(alignment: .leading, spacing: 2) {
                Text(verbatim: row.title).lineLimit(1).fontWeight(unread ? .semibold : .regular)
                if let detail = row.detail { Text(verbatim: detail).font(.footnote).lineLimit(2) }
            }
            Spacer()
            if marks.contains("attachment") { Image(systemName: "paperclip").font(.caption) }
            if marks.contains("starred") { Image(systemName: "star.fill").font(.caption).foregroundStyle(.yellow) }
            if marks.contains("pinned") { Image(systemName: "pin.fill").font(.caption) }
            if let meta = row.meta { Text(verbatim: meta).font(.caption).foregroundStyle(.secondary) }
        }
        .listRowInsets(EdgeInsets(top: 6, leading: 0, bottom: 6, trailing: Metrics.end))
        .contentShape(Rectangle())
        .onTapGesture { if let action = row.action { send(.tap(action)) } }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(row.action == nil ? [] : .isButton)
    }
}

/// A sheet a later wave draws (ROADMAP Wave 5 §8): its title, so that the frame is never blank.
struct Placeholder: View {
    let title: String
    let text: String

    var body: some View {
        VStack(spacing: 8) {
            Text(title).font(.title3)
            Text(verbatim: text).font(.footnote)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

/// The About screen: what the app is, then every bundled work's licence. The title is in the top
/// bar beside its close (MISC-13).
struct AboutView: View {
    let about: About

    var body: some View {
        List {
            Section {
                Text(verbatim: about.text)
            }
            ForEach(about.licences, id: \.name) { licence in
                DisclosureGroup(licence.name) {
                    Text(verbatim: licence.text).font(.footnote.monospaced()).textSelection(.enabled)
                }
            }
        }
        .accessibilityIdentifier("about")
    }
}
