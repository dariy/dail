import Darwin
import XCTest

@testable import Dail

/// The large demo (`large`: basic plus 20 000 inbox messages, app-testkit) in the simulator: how
/// long the host takes to open and sync it, how long a frame takes, and what the app's memory
/// footprint becomes (ROADMAP Wave 5 I3, plan §9). The numbers go to the log; the budgets only
/// catch a regression of an order of magnitude.
final class ScaleTests: XCTestCase {
    /// The app's physical footprint, as Xcode's memory gauge and the jetsam limit count it.
    private func footprint() -> UInt64 {
        var info = task_vm_info_data_t()
        var count = mach_msg_type_number_t(MemoryLayout<task_vm_info_data_t>.size / MemoryLayout<natural_t>.size)
        let result = withUnsafeMutablePointer(to: &info) {
            $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) {
                task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), $0, &count)
            }
        }
        return result == KERN_SUCCESS ? info.phys_footprint : 0
    }

    /// Runs the palette command `title`, answering a text step with `text` and a choice with its
    /// first row (the inbox comes first), until the palette closes.
    private func run(_ demo: DemoHost, _ title: String, text: String) throws {
        var frame = try demo.send(.configure(
            widthPt: 1180, heightPt: 820, dark: false, fontScale: 1, locale: "en-US", keyboard: false))
        let search = try XCTUnwrap(frame.topBar.actions.first { $0.icon == "search" })
        frame = try demo.send(.tap(search.action)) { $0.palette != nil }
        frame = try demo.send(.text(.query, title)) { $0.palette?.query.text == title }
        var row = frame.palette?.sections.flatMap(\.rows).first { $0.title == title }
        for _ in 0..<6 {
            guard let action = row?.action, let before = frame.palette else { break }
            // When the flow ends, the palette shows its first step until `close_palette` is
            // through: only a new question or a closed palette counts.
            frame = try demo.send(.tap(action)) {
                $0.palette == nil || ($0.palette?.prompt).map { $0 != before.prompt } == true
            }
            guard let sheet = frame.palette else { break }
            if sheet.prompt != nil, sheet.hint != "Filter…" {
                frame = try demo.send(.text(.query, text)) { $0.palette?.query.text == text }
            }
            row = frame.palette?.sections.first?.rows.first
        }
        XCTAssertNil(frame.palette, "\(title) ran")
    }

    func testTheLargeDemo() throws {
        let mib = { (bytes: UInt64) in Double(bytes) / 1_048_576 }
        let before = footprint()
        var start = Date()
        let demo = try DemoHost(backend: .object(["memory": .string("large")]))
        defer { demo.close() }
        let opened = Date().timeIntervalSince(start)
        let window = try XCTUnwrap(try demo.send(.tick) { ($0.list?.total ?? 0) > 0 }.list?.total)
        let early = footprint()

        // Keep the whole inbox offline: the rest is fetched in the background, 200 at a time.
        try run(demo, "Set how much mail to keep offline", text: "0")
        start = Date()
        let full = Int(LARGE_THREADS)
        let frame = try demo.send(.tick, timeout: 300) { ($0.list?.total ?? 0) >= full }
        let backfill = Date().timeIntervalSince(start)
        let total = frame.list?.total ?? 0
        start = Date()
        for first in stride(from: 0, to: total, by: total / 10 + 1) {
            _ = try demo.send(.scrolled(first: first, count: 12)) { $0.list?.offset ?? 0 <= first }
        }
        let scrolled = Date().timeIntervalSince(start)
        let after = footprint()
        print(String(
            format: "scale: open+first sync %.2f s (%d threads), backfill to %d threads %.1f s, 10 scrolls %.3f s, "
                + "footprint %.0f MiB before, %.0f after the first sync, %.0f with every thread",
            opened, window, total, backfill, scrolled, mib(before), mib(early), mib(after)))
        XCTAssertGreaterThanOrEqual(total, full, "the whole inbox is listed")
        XCTAssertLessThan(opened, 30)
        XCTAssertLessThan(backfill, 300)
        XCTAssertLessThan(scrolled, 10)
        // The iPad's jetsam limit is well above 1 GiB; the demo must stay far from it.
        XCTAssertLessThan(mib(after), 1_024)
    }
}

/// `large`'s threads: 20 000 messages in threads of 4, plus `basic`'s two.
private let LARGE_THREADS = 20_000 / 4 + 2
