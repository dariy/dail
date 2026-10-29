import Foundation
import UIKit

/// What the app shows: opening, a frame, or why the session could not open.
enum Screen {
    case opening
    case ready(Frame, FrameTheme?)
    case failed(String)
}

/// The session as Swift sees it, as Kotlin's `Host.kt`: every `open`, `event` and `frame` call
/// runs on one serial queue, "dail-ui", and the frames they return are published on the main
/// actor for SwiftUI to draw. Swift decides nothing: it sends what the user did.
final class Host: ObservableObject {
    @Published private(set) var screen: Screen = .opening
    private let ui = DispatchQueue(label: "dail-ui", qos: .userInteractive)
    /// Touched on `ui` only.
    private var handle: Int32 = 0
    private var listener: Listener?
    /// The theme of the last frame that carried one (frames carry it only when it changes).
    private var theme: FrameTheme?

    #if DEMO
    /// The memory backend over the `media` fixture: `basic` plus a photo and PDFs (app-ios's `demo` feature).
    static let backend: JSON = .object(["memory": .string("media")])
    #else
    static let backend: JSON = .string("live")
    #endif

    /// `blob` calls run here, one at a time, never on main or dail-ui (they block).
    private let blobs = DispatchQueue(label: "dail-blob", qos: .userInitiated)

    /// A `BlobRequest`'s bytes (`Native.blob`), or nil. Blocks the caller: call it off the main
    /// thread (a `CATiledLayer`'s draw, a task).
    func blob(_ request: JSON) -> Data? {
        blobs.sync {
            let handle = ui.sync { self.handle }
            guard handle > 0 else { return nil }
            return Native.blob(handle: handle, request: request.text)
        }
    }

    /// Opens the session on dail-ui, once, with the profile in Application Support.
    func open() {
        ui.async { [self] in
            guard handle == 0 else { return }
            let files = FileManager.default
            guard let support = try? files.url(
                for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
            else { return publish(.failed("no Application Support directory")) }
            let profile = support.appendingPathComponent("profiles/default", isDirectory: true)
            let outgoing = support.appendingPathComponent("outgoing", isDirectory: true)
            try? files.createDirectory(at: profile, withIntermediateDirectories: true)
            try? files.createDirectory(at: outgoing, withIntermediateDirectories: true)
            let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String
            let config: JSON = .object([
                "profile_dir": .string(profile.path),
                "cache_dir": .string(files.temporaryDirectory.path),
                "outgoing_dir": .string(outgoing.path),
                "backend": Self.backend,
                "lang": .string(Locale.preferredLanguages.first ?? "en"),
                "version": .string(version ?? ""),
            ])
            let listener = Listener(
                onWake: { [weak self] in self?.wake() },
                onAlert: { _ in }
            )
            let answer = (try? JSONDecoder().decode(JSON.self, from: Data(Native.open(config: config.text, listener: listener).utf8))) ?? .null
            guard case let .object(fields) = answer, case let .number(opened)? = fields["handle"] else {
                return publish(.failed(Self.error(answer)))
            }
            self.listener = listener
            handle = Int32(opened)
            take(Native.frame(handle: handle))
        }
    }

    /// Sends one event on dail-ui.
    func send(_ event: UiEvent) {
        ui.async { [self] in
            guard handle > 0 else { return }
            take(Native.event(handle: handle, event: event.json.text))
        }
    }

    /// A new frame is ready (called on a runtime thread).
    private func wake() {
        ui.async { [self] in
            guard handle > 0 else { return }
            take(Native.frame(handle: handle))
        }
    }

    private func take(_ json: String) {
        let data = Data(json.utf8)
        guard let answer = try? JSONDecoder().decode(Answer.self, from: data) else {
            let value = (try? JSONDecoder().decode(JSON.self, from: data)) ?? .string(json)
            return publish(.failed(Self.error(value)))
        }
        if let theme = answer.frame.theme { self.theme = theme }
        publish(.ready(answer.frame, theme))
        let externals = answer.externals ?? []
        if !externals.isEmpty {
            DispatchQueue.main.async { externals.forEach(Self.perform) }
        }
    }

    private func publish(_ screen: Screen) {
        DispatchQueue.main.async { self.screen = screen }
    }

    /// What only the host can do; the rest waits for later items (files, schedules).
    @MainActor
    private static func perform(_ external: External) {
        switch external.external {
        case "copy":
            if let text = external.args?.text { UIPasteboard.general.string = text }
        case "open_url":
            // Only after the user confirmed the link sheet, which showed the full target.
            if let url = external.args?.url.flatMap(URL.init(string:)) { UIApplication.shared.open(url) }
        case "haptic":
            UIImpactFeedbackGenerator(style: .light).impactOccurred()
        case "system_settings":
            if let url = URL(string: UIApplication.openSettingsURLString) { UIApplication.shared.open(url) }
        default:
            break
        }
    }

    private static func error(_ answer: JSON) -> String {
        if case let .object(fields) = answer, case let .string(error)? = fields["error"] { return error }
        return answer.text
    }
}
