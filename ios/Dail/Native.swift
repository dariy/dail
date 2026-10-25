import DailCore
import Foundation

/// The only file that touches the C ABI (`crates/app-ios/include/dail.h`): strings in and out,
/// freeing what Rust returns, and the listener's trampoline. Everything is JSON, as on Android.
enum Native {
    /// `dail_open`: `{"handle": n}` or `{"error": …}`. `listener` must stay alive until `close`.
    static func open(config: String, listener: Listener) -> String {
        let context = Unmanaged.passUnretained(listener).toOpaque()
        let native = DailListener(
            context: context,
            wake: { context in
                guard let context else { return }
                Unmanaged<Listener>.fromOpaque(context).takeUnretainedValue().wake()
            },
            alert: { context, json in
                guard let context, let json else { return }
                Unmanaged<Listener>.fromOpaque(context).takeUnretainedValue().alert(String(cString: json))
            }
        )
        return take(config.withCString { dail_open($0, native) })
    }

    /// `dail_event`: one `UiEvent`, answered with `{"frame": …, "externals": […]}` or `{"error": …}`.
    static func event(handle: Int32, event: String) -> String {
        take(event.withCString { dail_event(handle, $0) })
    }

    /// `dail_frame`: the current frame, after a wake.
    static func frame(handle: Int32) -> String {
        take(dail_frame(handle))
    }

    /// `dail_close`: shuts the session down; the listener is not called afterwards.
    static func close(handle: Int32) {
        dail_close(handle)
    }

    /// Copies a string Rust returned and hands it back to `dail_string_free`.
    private static func take(_ text: UnsafeMutablePointer<CChar>?) -> String {
        guard let text else { return #"{"error": "no answer"}"# }
        defer { dail_string_free(text) }
        return String(cString: text)
    }
}

/// Called by Rust from its runtime threads; both hop to the main actor.
final class Listener {
    let onWake: @Sendable () -> Void
    let onAlert: @Sendable (String) -> Void

    init(onWake: @escaping @Sendable () -> Void, onAlert: @escaping @Sendable (String) -> Void) {
        self.onWake = onWake
        self.onAlert = onAlert
    }

    func wake() { onWake() }
    func alert(_ json: String) { onAlert(json) }
}

/// The session behind one handle: opens it with the profile in Application Support, and keeps the
/// latest frame's title.
@MainActor
final class Host: ObservableObject {
    @Published private(set) var title = "dail"
    @Published private(set) var error: String?
    private var handle: Int32?
    private var listener: Listener?

    #if DEMO
    /// The memory backend over the `basic` fixture (app-ios's `demo` feature).
    static let backend: Any = ["memory": "basic"]
    #else
    static let backend: Any = "live"
    #endif

    func open() {
        guard handle == nil else { return }
        do {
            let support = try FileManager.default.url(
                for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
            let profile = support.appendingPathComponent("profiles/default", isDirectory: true)
            try FileManager.default.createDirectory(at: profile, withIntermediateDirectories: true)
            let config: [String: Any] = [
                "profile_dir": profile.path,
                "cache_dir": FileManager.default.temporaryDirectory.path,
                "backend": Self.backend,
                "lang": Locale.preferredLanguages.first ?? "en",
                "version": Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "",
            ]
            let json = String(decoding: try JSONSerialization.data(withJSONObject: config), as: UTF8.self)
            let listener = Listener(
                onWake: { [weak self] in Task { @MainActor in self?.refresh() } },
                onAlert: { _ in }
            )
            let answer = try Self.object(Native.open(config: json, listener: listener))
            guard let handle = (answer["handle"] as? NSNumber)?.int32Value else {
                throw HostError(message: answer["error"] as? String ?? "no handle")
            }
            self.listener = listener
            self.handle = handle
            refresh()
        } catch {
            self.error = "\(error)"
        }
    }

    /// Takes the current frame and shows its top bar's title.
    func refresh() {
        guard let handle else { return }
        do {
            let answer = try Self.object(Native.frame(handle: handle))
            guard let frame = answer["frame"] as? [String: Any],
                  let topBar = frame["top_bar"] as? [String: Any],
                  let title = topBar["title"] as? String
            else {
                throw HostError(message: answer["error"] as? String ?? "no frame")
            }
            self.title = title
            self.error = nil
        } catch {
            self.error = "\(error)"
        }
    }

    private static func object(_ json: String) throws -> [String: Any] {
        guard let object = try JSONSerialization.jsonObject(with: Data(json.utf8)) as? [String: Any] else {
            throw HostError(message: "not a JSON object: \(json)")
        }
        return object
    }
}

struct HostError: Error, CustomStringConvertible {
    let message: String
    var description: String { message }
}
