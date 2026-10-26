import Security
import XCTest

@testable import Dail

/// The palette from Swift, over frames only. With an empty query it lists every command the
/// presenter offers and the state allows (`app_front::palette::root_rows`; Rust's walk proves that
/// set covers the registry). This walk proves Swift reaches each of them: in a fresh demo host,
/// search the title, tap the row, answer its steps, until the palette closes.
final class PaletteTests: XCTestCase {
    private static let notCommands: Set<String> = ["Mailboxes", "Threads"]

    private func openPalette(_ demo: DemoHost) throws -> PaletteSheet {
        let frame = try demo.send(.configure(
            widthPt: 1180, heightPt: 820, dark: false, fontScale: 1, locale: "en-US", keyboard: false))
        let search = try XCTUnwrap(frame.topBar.actions.first { $0.icon == "search" }, "the search button")
        return try XCTUnwrap(try demo.send(.tap(search.action)) { $0.palette != nil }.palette, "the search button opens the palette")
    }

    /// What the user sees of the palette: a different value means the presenter answered.
    private struct Seen: Equatable {
        let prompt: String?
        let hint: String
        let rows: [String]

        init(_ sheet: PaletteSheet) {
            prompt = sheet.prompt
            hint = sheet.hint
            rows = sheet.sections.flatMap { $0.rows.map(\.title) }
        }
    }

    /// Every command row, found by its title and tapped, closes the palette, opens the setup or
    /// moves the palette to a step. What each step then builds is `app_front`'s, walked in Rust
    /// (`crates/app-mobile/tests/it/palette.rs`).
    func testEveryCommandIsReachableFromThePalette() throws {
        let titles = try {
            let demo = try DemoHost()
            defer { demo.close() }
            return try openPalette(demo).sections.filter { !Self.notCommands.contains($0.title) && $0.title != "Recent" }
                .flatMap { $0.rows.map(\.title) }
        }()
        XCTAssertGreaterThan(titles.count, 40, "the palette lists the commands")
        var unreached: [String] = []
        for title in titles {
            let demo = try DemoHost()
            defer { demo.close() }
            _ = try openPalette(demo)
            let typed = try demo.send(.text(.query, title)) { $0.palette?.query.text == title }
            guard let sheet = typed.palette,
                  let row = sheet.sections.filter({ !Self.notCommands.contains($0.title) }).flatMap(\.rows)
                      .first(where: { $0.title == title })
            else {
                unreached.append("\(title): not found by its title")
                continue
            }
            let before = Seen(sheet)
            let frame = try demo.send(.tap(row.action)) { frame in
                frame.setup != nil || frame.palette.map { Seen($0) != before } ?? true
            }
            if frame.setup == nil, let after = frame.palette, Seen(after) == before {
                unreached.append("\(title): the tap changed nothing")
            }
        }
        XCTAssertEqual(unreached, [], "commands Swift could not reach from the palette")
    }
}

/// A live session in the simulator: the setup stores the password in the Keychain (keyring's
/// protected-data store, `dail` / `account/<id>`), never in a frame.
final class KeychainTests: XCTestCase {
    private func passwords() -> [String: Data] {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: "dail",
            kSecMatchLimit as String: kSecMatchLimitAll,
            kSecReturnAttributes as String: true,
            kSecReturnData as String: true,
        ]
        var result: CFTypeRef?
        guard SecItemCopyMatching(query as CFDictionary, &result) == errSecSuccess,
              let items = result as? [[String: Any]]
        else { return [:] }
        var out: [String: Data] = [:]
        for item in items {
            if let user = item[kSecAttrAccount as String] as? String, let data = item[kSecValueData as String] as? Data {
                out[user] = data
            }
        }
        return out
    }

    func testTheSetupKeepsThePasswordInTheKeychain() throws {
        let before = passwords()
        let host = try DemoHost(backend: .string("live"))
        defer { host.close() }
        var frame = try host.send(.configure(
            widthPt: 390, heightPt: 844, dark: false, fontScale: 1, locale: "en-US", keyboard: false)) { $0.setup != nil }
        func button(_ label: String) throws -> ActionButton {
            try XCTUnwrap(frame.setup?.actions.first { $0.label == label }, "\(label) in \(frame.setup?.step ?? "no setup")")
        }
        XCTAssertNotNil(frame.setup, "a profile without accounts opens the setup")
        frame = try host.send(.tap(try button("Next").action)) { $0.setup?.step.hasPrefix("2") == true }
        frame = try host.send(.text(.email, "dail-ios-test@gmail.com")) { $0.setup?.actions.contains { $0.label == "Next" } == true }
        frame = try host.send(.tap(try button("Next").action)) { $0.setup?.fields.first?.input == .password }
        XCTAssertEqual(frame.setup?.fields.first?.input, .password)
        let password = "not-a-real-password-\(UUID().uuidString)"
        frame = try host.send(.text(.password, password)) { $0.setup?.actions.contains { $0.label == "Add and test" } == true }
        let shown = String(decoding: try JSONEncoder().encode(frame), as: UTF8.self)
        XCTAssertFalse(shown.contains(password), "the password never comes back in a frame")
        frame = try host.send(.tap(try button("Add and test").action))

        // The runtime stores it as the add's effect, just after the frame.
        var added: [String: Data] = [:]
        for _ in 0..<50 {
            added = passwords().filter { before[$0.key] == nil }
            if !added.isEmpty { break }
            Thread.sleep(forTimeInterval: 0.1)
        }
        defer {
            for user in added.keys {
                SecItemDelete([
                    kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: "dail",
                    kSecAttrAccount as String: user,
                ] as CFDictionary)
            }
        }
        XCTAssertEqual(added.count, 1, "one new Keychain item; the setup says: \(frame.setup?.error ?? frame.banner?.text ?? "nothing")")
        XCTAssertTrue(added.keys.first?.hasPrefix("account/") ?? false)
        XCTAssertEqual(added.values.first.map { String(decoding: $0, as: UTF8.self) }, password)
    }
}
