import SwiftUI

/// The app's entry: one window that draws the host's frames (ADR 0010). Rules stay in Rust; the
/// views only draw what `Host` publishes and send back what the user did.
@main
struct DailApp: App {
    @StateObject private var host = Host()
    @Environment(\.scenePhase) private var phase

    var body: some Scene {
        WindowGroup {
            RootView(host: host)
                .onAppear { host.open() }
                // SRV-QR2: a `dail://pair` link (a QR code, a page, another app) fills in the pairing
                // flow; Rust reads it and the user still taps Connect, then Pair. SwiftUI
                // delivers each URL once, so nothing is sent again when the view is redrawn.
                .onOpenURL { url in
                    if let event = UiEvent.pairLink(opening: url) { host.sendWhenOpen(event) }
                }
        }
        .onChange(of: phase) { _, phase in
            switch phase {
            case .active: host.send(.visible(true))
            case .background: host.send(.visible(false))
            default: break
            }
        }
    }
}
