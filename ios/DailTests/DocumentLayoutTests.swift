import XCTest

@testable import Dail

/// The preview's page geometry (the Swift port of the adapted `DocLayout`, MISC-19).
final class DocumentLayoutTests: XCTestCase {
    private let a4 = CGSize(width: 595, height: 842)
    private let landscape = CGSize(width: 842, height: 595)

    func testPagesStackFittedToTheWidestPage() {
        let layout = DocumentLayout(pages: [a4, landscape, a4], viewWidth: 1000, gap: 8)
        XCTAssertEqual(layout.rects[1].width, 1000, accuracy: 0.01)
        XCTAssertEqual(layout.rects[0].width, 595 * 1000 / 842, accuracy: 0.01)
        XCTAssertEqual(layout.rects[0].minX, (1000 - layout.rects[0].width) / 2, accuracy: 0.01)
        XCTAssertEqual(layout.rects[1].minY, layout.rects[0].maxY + 8, accuracy: 0.01)
        XCTAssertEqual(layout.size.height, layout.rects[2].maxY, accuracy: 0.01)
    }

    func testTheNearestPageFollowsTheRow() {
        let layout = DocumentLayout(pages: [a4, a4, a4], viewWidth: 595, gap: 10)
        XCTAssertEqual(layout.nearestPage(0), 0)
        XCTAssertEqual(layout.nearestPage(842 + 3), 0)
        XCTAssertEqual(layout.nearestPage(842 + 8), 1)
        XCTAssertEqual(layout.nearestPage(10_000), 2)
    }

    func testRenderHeightsRoundDown() {
        let layout = DocumentLayout(pages: [CGSize(width: 3, height: 10)], viewWidth: 300, gap: 0)
        XCTAssertEqual(layout.renderHeight(0, width: 4), 13)
    }
}
