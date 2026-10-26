import SwiftUI

/// Draws a frame as it is, as Kotlin's `DailScreen.kt`: the top bar (or the bulk bar), the banner,
/// the panes the layout names, the snackbar and the floating button, under the modal drawer of
/// compact windows. Sheets a later wave draws (about, preview, composer) show their title for now.
struct FrameView: View {
    let frame: Frame
    let palette: Palette
    let send: (UiEvent) -> Void

    var body: some View {
        VStack(spacing: 0) {
            TopBarView(frame: frame, palette: palette, send: send)
            if let banner = frame.banner { BannerView(banner: banner, send: send) }
            content
        }
        .overlay(alignment: .bottomTrailing) {
            if let fab = frame.fab, frame.snackbar == nil {
                SwiftUI.Button { send(.tap(fab.action)) } label: {
                    Label(fab.label, systemImage: Icons.symbol(fab.icon) ?? "square.and.pencil")
                        .padding(.horizontal, 18).padding(.vertical, 14)
                        .background(palette.role("primary_container", Color.accentColor.opacity(0.2)), in: Capsule())
                }
                .padding(20)
            }
        }
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
        if let sheet = frame.about {
            Placeholder(title: sheet.title, text: sheet.text)
        } else if let result = frame.result {
            ResultPane(result: result, send: send)
        } else if let sheet = frame.setup {
            SetupView(sheet: sheet, send: send)
        } else if let sheet = frame.palette {
            PaletteView(sheet: sheet, send: send)
        } else if let preview = frame.preview {
            Placeholder(title: preview.name, text: preview.detail)
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
                if let navigation = frame.topBar.navigation { FrameButton(button: navigation, send: send) }
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
        .padding(.horizontal, 16).padding(.vertical, 10)
        .foregroundStyle(palette.chrome("text_strong", Color.primary))
        .background(palette.chrome("base", Color(uiColor: .secondarySystemBackground)))
    }
}

/// The first problem and its fix (`app_front::status`).
struct BannerView: View {
    let banner: Banner
    let send: (UiEvent) -> Void

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: "exclamationmark.triangle")
            VStack(alignment: .leading) {
                Text(banner.text).fontWeight(.semibold)
                if let hint = banner.hint { Text(hint).font(.footnote) }
            }
            Spacer()
            if let action = banner.action { FrameButton(button: action, send: send) }
        }
        .padding(12)
        .background(Color.red.opacity(0.12), in: RoundedRectangle(cornerRadius: 12))
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

/// A query's result, read-only; email content in it is untrusted data, shown as plain text.
struct ResultPane: View {
    let result: ResultView
    let send: (UiEvent) -> Void

    var body: some View {
        VStack(alignment: .leading) {
            HStack {
                Text(result.title).font(.title3)
                Spacer()
                FrameButton(button: result.close, send: send)
            }
            Text(result.note).font(.footnote)
            ScrollView {
                LazyVStack(alignment: .leading) {
                    ForEach(Array(result.lines.enumerated()), id: \.offset) { _, line in
                        Text(verbatim: line).font(.body.monospaced()).textSelection(.enabled)
                    }
                }
            }
        }
        .padding()
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
