import SwiftUI

/// The app's entry: one window that shows the host's frames (ADR 0010). Rules stay in Rust; this
/// file and the views only draw what `Native` hands them.
@main
struct DailApp: App {
    @StateObject private var host = Host()

    var body: some Scene {
        WindowGroup {
            ContentView(host: host)
        }
    }
}

/// The first frame's title, for now (ROADMAP I2): the views that draw whole frames come in I3.
struct ContentView: View {
    @ObservedObject var host: Host

    var body: some View {
        NavigationStack {
            Group {
                if let error = host.error {
                    Text(error).foregroundStyle(.red).padding()
                } else {
                    ProgressView()
                }
            }
            .navigationTitle(host.title)
        }
        .onAppear { host.open() }
    }
}
