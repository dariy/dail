import XCTest

@testable import Dail

/// The demo host (app-ios with `demo`) in the simulator: it opens, answers a frame Swift decodes
/// strictly, takes an event and closes.
final class HostTests: XCTestCase {
    func testTheDemoHostAnswersAFrame() throws {
        let profile = FileManager.default.temporaryDirectory
            .appendingPathComponent("dail-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: profile, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: profile) }
        let config: JSON = .object([
            "profile_dir": .string(profile.path),
            "backend": .object(["memory": .string("basic")]),
            "lang": .string("en"),
        ])
        let listener = Listener(onWake: {}, onAlert: { _ in })
        let opened = try JSONDecoder().decode(JSON.self, from: Data(Native.open(config: config.text, listener: listener).utf8))
        guard case let .object(answer) = opened, case let .number(number)? = answer["handle"] else {
            return XCTFail("no handle: \(opened.text)")
        }
        let handle = Int32(number)
        defer { Native.close(handle: handle) }

        let first: Answer = try strictly("frame", Data(Native.frame(handle: handle).utf8))
        XCTAssertGreaterThan(first.frame.revision, 0)
        XCTAssertFalse(first.frame.topBar.title.isEmpty)

        let configure = UiEvent.configure(
            widthPt: 1180, heightPt: 820, dark: false, fontScale: 1, locale: "en-US", keyboard: false)
        let next: Answer = try strictly("configure", Data(Native.event(handle: handle, event: configure.json.text).utf8))
        XCTAssertGreaterThan(next.frame.revision, first.frame.revision)
        XCTAssertEqual(next.frame.layout.windowClass, .expanded)
        withExtendedLifetime(listener) {}
    }
}
