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
