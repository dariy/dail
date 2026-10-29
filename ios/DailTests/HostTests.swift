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
            widthPt: 1180, heightPt: 820, dark: false, fontScale: 1, locale: "en-US", keyboard: false)) { $0.list?.rows.isEmpty == false }
        let list = try XCTUnwrap(frame.list)
        let total = list.total
        XCTAssertGreaterThan(total, 0)
        let first = try XCTUnwrap(list.rows.first)

        frame = try demo.send(.tap(first.action)) { $0.reader != nil }
        let reader = try XCTUnwrap(frame.reader, "a tap on a row opens the thread")
        XCTAssertEqual(reader.thread, first.thread)

        let star = try XCTUnwrap(reader.actions.first { $0.icon == "star" })
        frame = try demo.send(.tap(star.action)) { $0.list?.rows.first { $0.thread == first.thread }?.starred != first.starred }
        XCTAssertEqual(frame.list?.rows.first { $0.thread == first.thread }?.starred, !first.starred)

        let archive = try XCTUnwrap(frame.reader?.actions.first { $0.icon == "archive" })
        frame = try demo.send(.tap(archive.action)) { $0.list?.total == total - 1 }
        XCTAssertEqual(frame.list?.total, total - 1, "archived threads leave the inbox")
        let undo = try XCTUnwrap(frame.snackbar?.action, "the snackbar offers undo")

        frame = try demo.send(.tap(undo.action)) { $0.list?.total == total }
        XCTAssertEqual(frame.list?.total, total, "undo brings the thread back")
        XCTAssertTrue(frame.list?.rows.contains { $0.thread == first.thread } ?? false)
    }

    /// About closes from its top bar, and gives way to a mailbox opened from the drawer (MISC-12).
    func testAboutClosesAndGivesWayToTheDrawer() throws {
        let demo = try DemoHost()
        defer { demo.close() }
        var frame = try demo.send(.configure(
            widthPt: 1180, heightPt: 820, dark: false, fontScale: 1, locale: "en-US", keyboard: false)) { $0.list?.rows.isEmpty == false }
        let about = try XCTUnwrap(frame.drawer?.footer.first { $0.label == "About" })

        frame = try demo.send(.tap(about.action)) { $0.about != nil }
        let close = try XCTUnwrap(frame.topBar.navigation, "About draws its close")
        frame = try demo.send(.tap(close.action)) { $0.about == nil }
        XCTAssertNil(frame.about)
        XCTAssertNotNil(frame.list, "the list is visible again")

        let again = try XCTUnwrap(frame.drawer?.footer.first { $0.label == "About" })
        frame = try demo.send(.tap(again.action)) { $0.about != nil }
        let drawer = try XCTUnwrap(frame.drawer, "the drawer stays beside About on a wide window")
        let mailbox = try XCTUnwrap(drawer.accounts.first?.mailboxes.first { $0.icon == "send" })
        frame = try demo.send(.tap(mailbox.action)) { $0.about == nil }
        XCTAssertNil(frame.about, "About is gone")
        XCTAssertNotNil(frame.list)
    }
}

/// The demo host for a test, in its own profile.
extension HostTests {
    /// MISC-19: the media fixture's photo comes back upright and the PDF as pages and tiles of the
    /// asked sizes, through `dail_blob`.
    func testMediaPreviewBlobs() throws {
        let demo = try DemoHost(backend: .object(["memory": .string("media")]))
        defer { demo.close() }
        var frame = try demo.send(.configure(
            widthPt: 1180, heightPt: 820, dark: false, fontScale: 1, locale: "en-US", keyboard: false)) { $0.list?.rows.isEmpty == false }
        let row = try XCTUnwrap(frame.list?.rows.first { $0.subject.contains("Photos and the report") })
        frame = try demo.send(.tap(row.action), timeout: 10) { $0.reader?.messages.contains { !$0.attachments.isEmpty } == true }
        let message = try XCTUnwrap(frame.reader?.messages.first { !$0.attachments.isEmpty })
        frame = try demo.send(.tap(message.attachments[1].open)) { $0.preview != nil }
        let preview = try XCTUnwrap(frame.preview)
        XCTAssertEqual(preview.kind, .pdf)

        func blob(_ request: JSON) -> Data? { Native.blob(handle: demo.handle, request: request.text) }
        let at: (Int) -> [String: JSON] = { ["blob": .string(preview.blob), "part": .number(Double($0))] }
        let pages = try XCTUnwrap(blob(.object(["pdf_pages": .object(at(preview.part))])))
        guard case let .object(answer) = try JSONDecoder().decode(JSON.self, from: pages), case let .array(list)? = answer["pages"] else {
            return XCTFail("no pages")
        }
        XCTAssertEqual(list.count, 3)
        var page = at(preview.part)
        page["page"] = .number(0)
        page["width"] = .number(600)
        XCTAssertEqual(blob(.object(["pdf_page": .object(page)])).flatMap(rgbaImage)?.width, 600)
        page["width"] = .number(4000)
        page["tile"] = .object(["x": .number(100), "y": .number(200), "w": .number(512), "h": .number(256)])
        let tile = try XCTUnwrap(blob(.object(["pdf_page": .object(page)])).flatMap(rgbaImage))
        XCTAssertEqual([tile.width, tile.height], [512, 256])
        let photo = try XCTUnwrap(blob(.object(["image": .object(at(message.attachments[0].part))])).flatMap(rgbaImage))
        XCTAssertGreaterThan(photo.height, photo.width, "upright: portrait")
    }
}

final class DemoHost {
    let handle: Int32
    let profile: URL
    let listener = Listener(onWake: {}, onAlert: { _ in })

    init(backend: JSON = .object(["memory": .string("basic")])) throws {
        profile = FileManager.default.temporaryDirectory
            .appendingPathComponent("dail-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: profile, withIntermediateDirectories: true)
        let config: JSON = .object([
            "profile_dir": .string(profile.path),
            "backend": backend,
            "lang": .string("en"),
        ])
        let opened = try JSONDecoder().decode(
            JSON.self, from: Data(Native.open(config: config.text, listener: listener).utf8))
        guard case let .object(answer) = opened, case let .number(number)? = answer["handle"] else {
            throw DemoError(message: "no handle: \(opened.text)")
        }
        handle = Int32(number)
    }

    /// Sends `event` and decodes the answer strictly. Commands run on the session's runtime, so the
    /// answer may come before their effect: `until` waits (up to `timeout`) for the frame that shows it,
    /// taking frames as the app does after a wake.
    func send(_ event: UiEvent, timeout: TimeInterval = 5, until done: (Frame) -> Bool = { _ in true }) throws -> Frame {
        let answer: Answer = try strictly("\(event)", Data(Native.event(handle: handle, event: event.json.text).utf8))
        var frame = answer.frame
        let deadline = Date().addingTimeInterval(timeout)
        while !done(frame), Date() < deadline {
            Thread.sleep(forTimeInterval: timeout > 5 ? 0.25 : 0.02)
            let next: Answer = try strictly("frame", Data(Native.frame(handle: handle).utf8))
            frame = next.frame
        }
        return frame
    }

    func close() {
        Native.close(handle: handle)
        try? FileManager.default.removeItem(at: profile)
    }
}

struct DemoError: Error { let message: String }
