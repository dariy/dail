import Foundation

/// One event for the presenter (`app_mobile::UiEvent`): `{"event": …, "args": {…}}`. Swift never
/// builds a command; it returns the action IDs of the frame it shows, as Kotlin's `UiEvent.kt`.
enum UiEvent {
    case configure(widthPt: Int, heightPt: Int, dark: Bool, fontScale: Double, locale: String?, keyboard: Bool)
    case visible(Bool)
    case tap(ActionId)
    case longPress(ActionId)
    case swipe(ActionId)
    /// A pinch on the message ended: fingers apart (zoom in) or together (MISC-6).
    case pinch(zoomIn: Bool)
    case scrolled(first: Int, count: Int)
    case back
    case refresh
    case online
    /// A tap on a notification: its `open`, passed back verbatim.
    case notification(act: JSON)
    /// A hardware key no text field took, in Android's key codes; Rust maps it to a chord.
    case keyDown(code: Int, unicode: Int, ctrl: Bool, shift: Bool, alt: Bool, meta: Bool)
    case text(Field, String)
    case blur(Field)
    case key(chord: String)
    case tick
    /// A `mailto:` link or a share from another app; Rust decides what the draft gets.
    case share(mailto: String?, to: [String], subject: String?, text: String?, files: [String])

    var json: JSON {
        switch self {
        case let .configure(width, height, dark, fontScale, locale, keyboard):
            var args: [String: JSON] = [
                "width_dp": .number(Double(width)), "height_dp": .number(Double(height)), "dark": .bool(dark),
                "font_scale": .number(fontScale), "keyboard": .bool(keyboard),
            ]
            if let locale { args["locale"] = .string(locale) }
            return Self.event("configure", args)
        case let .visible(visible): return Self.event("visible", ["visible": .bool(visible)])
        case let .tap(action): return Self.event("tap", ["action": .number(Double(action))])
        case let .longPress(action): return Self.event("long_press", ["action": .number(Double(action))])
        case let .swipe(action): return Self.event("swipe", ["action": .number(Double(action))])
        case let .pinch(zoomIn): return Self.event("pinch", ["zoom_in": .bool(zoomIn)])
        case let .scrolled(first, count):
            return Self.event("scrolled", ["first": .number(Double(first)), "count": .number(Double(count))])
        case .back: return Self.event("back")
        case .refresh: return Self.event("refresh")
        case .online: return Self.event("online")
        case let .notification(act): return Self.event("notification", ["act": act])
        case let .keyDown(code, unicode, ctrl, shift, alt, meta):
            return Self.event("key_down", [
                "code": .number(Double(code)), "unicode": .number(Double(unicode)), "ctrl": .bool(ctrl),
                "shift": .bool(shift), "alt": .bool(alt), "meta": .bool(meta),
            ])
        case let .text(field, text):
            return Self.event("text", ["field": .string(field.rawValue), "text": .string(text)])
        case let .blur(field): return Self.event("blur", ["field": .string(field.rawValue)])
        case let .key(chord): return Self.event("key", ["chord": .string(chord)])
        case .tick: return Self.event("tick")
        case let .share(mailto, to, subject, text, files):
            var args: [String: JSON] = [:]
            if let mailto { args["mailto"] = .string(mailto) }
            if !to.isEmpty { args["to"] = .array(to.map(JSON.string)) }
            if let subject { args["subject"] = .string(subject) }
            if let text { args["text"] = .string(text) }
            if !files.isEmpty { args["files"] = .array(files.map(JSON.string)) }
            return Self.event("share", args)
        }
    }

    private static func event(_ name: String, _ args: [String: JSON]? = nil) -> JSON {
        var event: [String: JSON] = ["event": .string(name)]
        if let args { event["args"] = .object(args) }
        return .object(event)
    }
}
