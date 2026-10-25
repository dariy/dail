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

    /// Triage over frames and events only, as the views do it: open the first thread, mark it,
    /// archive it, and undo from the snackbar.
    func testTriageWithUndoThroughFrames() throws {
        let demo = try DemoHost()
        defer { demo.close() }
        var frame = try demo.send(.configure(
            widthPt: 1180, heightPt: 820, dark: false, fontScale: 1, locale: "en-US", keyboard: false))
        let list = try XCTUnwrap(frame.list)
        let total = list.total
        XCTAssertGreaterThan(total, 0)
        let first = try XCTUnwrap(list.rows.first)

        frame = try demo.send(.tap(first.action))
        let reader = try XCTUnwrap(frame.reader, "a tap on a row opens the thread")
        XCTAssertEqual(reader.thread, first.thread)

        let star = try XCTUnwrap(reader.actions.first { $0.icon == "star" })
        frame = try demo.send(.tap(star.action))
        XCTAssertEqual(frame.list?.rows.first { $0.thread == first.thread }?.starred, !first.starred)

        let archive = try XCTUnwrap(frame.reader?.actions.first { $0.icon == "archive" })
        frame = try demo.send(.tap(archive.action))
        XCTAssertEqual(frame.list?.total, total - 1, "archived threads leave the inbox")
        let undo = try XCTUnwrap(frame.snackbar?.action, "the snackbar offers undo")

        frame = try demo.send(.tap(undo.action))
        XCTAssertEqual(frame.list?.total, total, "undo brings the thread back")
        XCTAssertTrue(frame.list?.rows.contains { $0.thread == first.thread } ?? false)
    }
}

/// The demo host for a test, in its own profile.
final class DemoHost {
    let handle: Int32
    let profile: URL
    let listener = Listener(onWake: {}, onAlert: { _ in })

    init() throws {
        profile = FileManager.default.temporaryDirectory
            .appendingPathComponent("dail-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: profile, withIntermediateDirectories: true)
        let config: JSON = .object([
            "profile_dir": .string(profile.path),
            "backend": .object(["memory": .string("basic")]),
            "lang": .string("en"),
        ])
        let opened = try JSONDecoder().decode(
            JSON.self, from: Data(Native.open(config: config.text, listener: listener).utf8))
        guard case let .object(answer) = opened, case let .number(number)? = answer["handle"] else {
            throw DemoError(message: "no handle: \(opened.text)")
        }
        handle = Int32(number)
    }

    /// Sends `event` and decodes the answer strictly.
    func send(_ event: UiEvent) throws -> Frame {
        let answer: Answer = try strictly("\(event)", Data(Native.event(handle: handle, event: event.json.text).utf8))
        return answer.frame
    }

    func close() {
        Native.close(handle: handle)
        try? FileManager.default.removeItem(at: profile)
    }
}

struct DemoError: Error { let message: String }
