import XCTest

@testable import Dail

/// The contract fixtures `app-mobile`'s tests write (`crates/app-mobile/tests/it/contract.rs`), the
/// same files Kotlin's `ContractTest` reads. Codable ignores unknown keys, so "strictly" means: the
/// fixture decodes, and re-encoding the value gives back every key and value the fixture had.
final class ContractTests: XCTestCase {
    private func fixtures(_ kind: String) throws -> [String: Data] {
        let bundle = Bundle(for: ContractTests.self)
        let dir = try XCTUnwrap(bundle.url(forResource: "contract", withExtension: nil)).appendingPathComponent(kind)
        let files = try FileManager.default.contentsOfDirectory(at: dir, includingPropertiesForKeys: nil)
        var fixtures: [String: Data] = [:]
        for file in files where file.pathExtension == "json" {
            fixtures[file.deletingPathExtension().lastPathComponent] = try Data(contentsOf: file)
        }
        return fixtures
    }

    func testEveryFrameDecodesStrictly() throws {
        let frames = try fixtures("frames")
        XCTAssertFalse(frames.isEmpty)
        for (name, data) in frames {
            let frame: Frame = try strictly(name, data)
            XCTAssertGreaterThan(frame.revision, 0, name)
        }
    }

    func testEveryExternalDecodesStrictly() throws {
        for (name, data) in try fixtures("externals") {
            let external: External = try strictly(name, data)
            XCTAssertEqual(external.external, name)
        }
    }

    func testEveryNotificationDecodesStrictly() throws {
        let notes = try fixtures("notifications")
        XCTAssertEqual(Set(notes.keys), ["new_mail", "not_sent", "push"])
        for (name, data) in notes {
            let note: Note = try strictly(name, data)
            XCTAssertEqual(note.tag == "push" ? "push" : note.channel, name)
            XCTAssertFalse(note.publicTitle.isEmpty, name)
        }
    }

    /// Swift's events, built as the app builds them, equal Rust's fixtures; a new one fails here.
    func testEveryEventEncodesAsRustExpects() throws {
        let swift: [String: UiEvent] = [
            "back": .back,
            "blur": .blur(.to),
            "configure": .configure(
                widthPt: 360, heightPt: 800, dark: true, fontScale: 1.25, locale: "en-US", keyboard: false),
            "key": .key(chord: "Ctrl+Z"),
            "key_down": .keyDown(code: 33, unicode: 101, ctrl: false, shift: false, alt: false, meta: false),
            "long_press": .longPress(8),
            "refresh": .refresh,
            "online": .online,
            "notification": .notification(
                act: .object(["act": .string("open_thread"), "thread": .string("00000000-0000-0000-0000-000000000009")])),
            "scrolled": .scrolled(first: 20, count: 12),
            "share": .share(
                mailto: "mailto:bob@example.org?subject=Hi", to: ["carol@example.org"], subject: "Photos",
                text: "Here", files: ["content://com.example.files/a.pdf"]),
            "swipe": .swipe(9),
            "tap": .tap(7),
            "text": .text(.subject, "Lunch?"),
            "tick": .tick,
            "visible": .visible(true),
        ]
        let rust = try fixtures("events")
        XCTAssertEqual(Set(rust.keys), Set(swift.keys))
        for (name, data) in rust {
            XCTAssertEqual(try JSONDecoder().decode(JSON.self, from: data), swift[name]?.json, name)
        }
    }
}

/// Decodes `data` as `T` and fails unless the value, encoded again, holds all of `data`.
func strictly<T: Codable>(_ name: String, _ data: Data) throws -> T {
    let value = try JSONDecoder().decode(T.self, from: data)
    let original = try JSONDecoder().decode(JSON.self, from: data)
    let again = try JSONDecoder().decode(JSON.self, from: JSONEncoder().encode(value))
    if let path = lost(original, again, at: "$") {
        XCTFail("\(name): \(T.self) drops or changes \(path)")
    }
    return value
}

/// The first path where `original` has something `again` lacks; `null` counts as absent.
private func lost(_ original: JSON, _ again: JSON?, at path: String) -> String? {
    switch (original, again) {
    case (.null, nil), (.null, .null?):
        return nil
    case let (.object(object), .object(other)?):
        for (key, value) in object.sorted(by: { $0.key < $1.key }) {
            if let path = lost(value, other[key], at: "\(path).\(key)") { return path }
        }
        return nil
    case let (.array(array), .array(other)?) where array.count == other.count:
        for (index, (value, encoded)) in zip(array, other).enumerated() {
            if let path = lost(value, encoded, at: "\(path)[\(index)]") { return path }
        }
        return nil
    default:
        return original == again ? nil : path
    }
}
