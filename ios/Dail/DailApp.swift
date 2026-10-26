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
