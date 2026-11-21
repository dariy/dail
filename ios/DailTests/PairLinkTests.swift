import XCTest

@testable import Dail

/// SRV-QR2: which opened URL becomes `pair_link`; the link itself is read in Rust.
final class PairLinkTests: XCTestCase {
    private func event(_ text: String) -> JSON? {
        URL(string: text).flatMap(UiEvent.pairLink(opening:))?.json
    }

    func testAPairLinkIsSentAsItCame() {
        let link = "dail://pair?v=1&h=home.example&p=7443&c=ABCD2345&fp=00&n=Laptop"
        XCTAssertEqual(event(link), UiEvent.pairLink(link).json)
        XCTAssertEqual(event("DAIL://PAIR?v=1"), UiEvent.pairLink("DAIL://PAIR?v=1").json)
        XCTAssertEqual(event("dail://pair"), UiEvent.pairLink("dail://pair").json)
    }

    func testOtherURLsAreIgnored() {
        XCTAssertNil(event("dail://open?x=1"))
        XCTAssertNil(event("dail://pairing?v=1"))
        XCTAssertNil(event("mailto:bob@example.org"))
        XCTAssertNil(event("https://example.org/dail://pair"))
    }
}
