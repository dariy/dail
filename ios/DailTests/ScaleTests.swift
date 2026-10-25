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

    func testTheLargeDemo() throws {
        let mib = { (bytes: UInt64) in Double(bytes) / 1_048_576 }
        let before = footprint()
        var start = Date()
        let demo = try DemoHost(backend: .object(["memory": .string("large")]))
        defer { demo.close() }
        let opened = Date().timeIntervalSince(start)
        start = Date()
        let frame = try demo.send(.configure(
            widthPt: 1180, heightPt: 820, dark: false, fontScale: 1, locale: "en-US", keyboard: false)) { ($0.list?.total ?? 0) > 1_000 }
        let firstFrame = Date().timeIntervalSince(start)
        let total = frame.list?.total ?? 0
        start = Date()
        for first in stride(from: 0, to: min(total, 5_000), by: 500) {
            _ = try demo.send(.scrolled(first: first, count: 12))
        }
        let scrolled = Date().timeIntervalSince(start)
        let after = footprint()
        print(String(
            format: "scale: open+sync %.2f s, first full frame %.2f s, 10 scrolls %.2f s, %d threads, footprint %.0f → %.0f MiB",
            opened, firstFrame, scrolled, total, mib(before), mib(after)))
        XCTAssertGreaterThan(total, 1_000, "the large inbox is listed")
        XCTAssertLessThan(opened + firstFrame, 60)
        XCTAssertLessThan(scrolled, 10)
        // The iPad's jetsam limit is well above 1 GiB; the demo must stay far from it.
        XCTAssertLessThan(mib(after), 1_024)
    }
}
