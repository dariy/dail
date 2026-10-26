import SwiftUI

/// Opening, failed, or the frame; and `configure` whenever the window or the system's look changes.
struct RootView: View {
    @ObservedObject var host: Host
    @Environment(\.colorScheme) private var scheme
    @Environment(\.dynamicTypeSize) private var typeSize

    var body: some View {
        GeometryReader { geometry in
            Group {
                switch host.screen {
                case .opening:
                    ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
                case let .failed(error):
                    Text(error).padding(24).frame(maxWidth: .infinity, maxHeight: .infinity)
                case let .ready(frame, theme):
                    FrameView(frame: frame, palette: Palette(theme: theme), send: host.send)
                }
            }
            .onAppear { configure(geometry.size) }
            .onChange(of: geometry.size) { _, size in configure(size) }
            .onChange(of: scheme) { _, _ in configure(geometry.size) }
            .onChange(of: typeSize) { _, _ in configure(geometry.size) }
        }
    }

    /// Points are iOS's density-independent unit, as dp are Android's.
    private func configure(_ size: CGSize) {
        host.send(
            .configure(
                widthPt: Int(size.width), heightPt: Int(size.height), dark: scheme == .dark,
                fontScale: typeSize.scale, locale: Locale.preferredLanguages.first, keyboard: false))
    }
}

extension DynamicTypeSize {
    /// The font scale `configure` reports, from the system's text size (large = 1).
    var scale: Double {
        switch self {
        case .xSmall: 0.82
        case .small: 0.88
        case .medium: 0.94
        case .large: 1
        case .xLarge: 1.12
        case .xxLarge: 1.24
        case .xxxLarge: 1.35
        case .accessibility1: 1.65
        case .accessibility2: 1.94
        case .accessibility3: 2.35
        case .accessibility4: 2.76
        case .accessibility5: 3.12
        @unknown default: 1
        }
    }
}
