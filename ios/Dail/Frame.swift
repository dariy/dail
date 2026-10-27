import Foundation

// The screen models of `app_mobile::frame`, by hand, as Kotlin's `ui/Frame.kt`. `DailTests` decodes
// every contract fixture and checks that nothing in it was dropped, so a change in Rust that Swift
// does not follow fails CI. Snake-case keys are spelled out in `CodingKeys`: a key strategy would
// also rename the keys of the theme's maps.

typealias ActionId = Int

/// Any JSON value: what Swift passes back verbatim (notification acts) or never reads.
enum JSON: Codable, Equatable {
    case null
    case bool(Bool)
    case number(Double)
    case string(String)
    case array([JSON])
    case object([String: JSON])

    init(from decoder: Decoder) throws {
        let value = try decoder.singleValueContainer()
        if value.decodeNil() {
            self = .null
        } else if let bool = try? value.decode(Bool.self) {
            self = .bool(bool)
        } else if let number = try? value.decode(Double.self) {
            self = .number(number)
        } else if let string = try? value.decode(String.self) {
            self = .string(string)
        } else if let array = try? value.decode([JSON].self) {
            self = .array(array)
        } else {
            self = .object(try value.decode([String: JSON].self))
        }
    }

    func encode(to encoder: Encoder) throws {
        var value = encoder.singleValueContainer()
        switch self {
        case .null: try value.encodeNil()
        case let .bool(bool): try value.encode(bool)
        case let .number(number): try value.encode(number)
        case let .string(string): try value.encode(string)
        case let .array(array): try value.encode(array)
        case let .object(object): try value.encode(object)
        }
    }

    /// The value as compact JSON text.
    var text: String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        return (try? encoder.encode(self)).map { String(decoding: $0, as: UTF8.self) } ?? "null"
    }
}

/// What `event` and `frame` answer.
struct Answer: Codable {
    let frame: Frame
    let externals: [External]?
}

struct Frame: Codable {
    let revision: Int64
    let layout: FrameLayout
    let topBar: TopBar
    let drawer: Drawer?
    let list: ThreadList?
    let reader: Reader?
    let composer: Composer?
    let settings: SettingsPage?
    let setup: SetupSheet?
    let bulk: Bulk?
    let fab: ActionButton?
    let banner: Banner?
    let snackbar: Snackbar?
    let theme: FrameTheme?
    let about: About?
    let palette: PaletteSheet?
    let result: ResultView?
    let link: LinkSheet?
    let preview: AttachmentPreview?
    let back: Bool
    let wantsTick: Bool

    enum CodingKeys: String, CodingKey {
        case revision, layout, drawer, list, reader, composer, settings, setup, bulk, fab, banner, snackbar
        case theme, about, palette, result, link, preview, back
        case topBar = "top_bar"
        case wantsTick = "wants_tick"
    }
}

enum WindowClass: String, Codable { case compact, medium, expanded }

enum PaneKind: String, Codable { case drawer, list, reader, composer, settings }

struct FrameLayout: Codable {
    let windowClass: WindowClass
    let panes: [PaneKind]

    enum CodingKeys: String, CodingKey {
        case panes
        case windowClass = "class"
    }
}

struct ActionButton: Codable {
    let icon: String?
    let label: String
    let action: ActionId
}

struct TopBar: Codable {
    let title: String
    let subtitle: String?
    let navigation: ActionButton?
    let actions: [ActionButton]
}

struct Drawer: Codable {
    let modal: Bool
    let rail: Bool
    let allInboxes: DrawerItem?
    let accounts: [DrawerAccount]
    let footer: [DrawerItem]

    enum CodingKeys: String, CodingKey {
        case modal, rail, accounts, footer
        case allInboxes = "all_inboxes"
    }
}

struct DrawerAccount: Codable {
    let name: String
    let email: String
    let tag: Int?
    let mailboxes: [DrawerItem]
}

struct DrawerItem: Codable {
    let label: String
    let icon: String
    let unread: Int
    let selected: Bool
    let action: ActionId
    let a11y: String
}

enum Density: String, Codable { case cozy, compact, preview }

struct ThreadList: Codable {
    let title: String
    let total: Int
    let offset: Int
    let density: Density
    let checked: Int
    let refreshing: Bool
    let rows: [Row]
    let server: String?
    let empty: String?
}

struct Row: Codable {
    let thread: String
    let action: ActionId
    let longPress: ActionId
    let swipeStart: ActionId
    let swipeEnd: ActionId
    let sender: String
    let subject: String
    let snippet: String
    let date: String
    let unread: Bool
    let starred: Bool
    /// Pinned to the top (MISC-4); absent when not.
    let pinned: Bool?
    let attachments: Bool
    let messages: Int
    let tag: Int?
    let selected: Bool
    let checked: Bool
    let marks: RowMarks?
    let a11y: String

    enum CodingKeys: String, CodingKey {
        case thread, action, sender, subject, snippet, date, unread, starred, pinned, attachments, messages, tag
        case selected, checked, marks, a11y
        case longPress = "long_press"
        case swipeStart = "swipe_start"
        case swipeEnd = "swipe_end"
    }
}

/// A piece of text cut where the search words are.
struct Piece: Codable {
    let text: String
    let hit: Bool?
}

struct RowMarks: Codable {
    let subject: [Piece]
    let snippet: [Piece]
}

struct Bulk: Codable {
    let title: String
    let actions: [ActionButton]
    let close: ActionButton
}

struct Reader: Codable {
    let thread: String
    let subject: String
    let close: ActionButton
    let actions: [ActionButton]
    let previous: ActionButton?
    let next: ActionButton?
    let position: String?
    let messages: [ReaderMessage]
}

struct ReaderMessage: Codable {
    let message: String
    let from: String
    let to: String
    let date: String
    let expanded: Bool
    let toggle: ActionId
    let snippet: String
    let body: [DocBlock]?
    let status: String?
    let attachments: [Attachment]
    let saveAll: ActionButton?
    let images: ImagesBar?
    let links: [ActionButton]

    enum CodingKeys: String, CodingKey {
        case message, from, to, date, expanded, toggle, snippet, body, status, attachments, images, links
        case saveAll = "save_all"
    }
}

struct ImagesBar: Codable {
    let shown: Bool
    let text: String
    let toggle: ActionButton
}

/// A picture carried in the message: `blob`'s `inline` request.
struct InlineImage: Codable {
    let blob: String
    let cid: String
}

enum PreviewKind: String, Codable { case image, text, none }

/// A received attachment shown in the app; other apps get it only through Save.
struct AttachmentPreview: Codable {
    let name: String
    let detail: String
    let kind: PreviewKind
    let blob: String
    let part: Int
    let note: String?
    let save: ActionButton
    let close: ActionButton
}

/// A block of a message body (`app_mobile::frame::DocBlock`), by its `type`.
indirect enum DocBlock: Codable {
    case paragraph(spans: [DocSpan])
    case heading(level: Int, spans: [DocSpan])
    case list(ordered: Bool, start: Int, items: [[DocBlock]])
    case quote(blocks: [DocBlock])
    case preformatted(lines: [String])
    case table(rows: [[DocCell]])
    case rule
    case fold(label: String, expanded: Bool, toggle: ActionId, blocks: [DocBlock])

    enum CodingKeys: String, CodingKey {
        case type, spans, level, ordered, start, items, blocks, lines, rows, label, expanded, toggle
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        switch try c.decode(String.self, forKey: .type) {
        case "paragraph":
            self = .paragraph(spans: try c.decode([DocSpan].self, forKey: .spans))
        case "heading":
            self = .heading(
                level: try c.decode(Int.self, forKey: .level), spans: try c.decode([DocSpan].self, forKey: .spans))
        case "list":
            self = .list(
                ordered: try c.decode(Bool.self, forKey: .ordered),
                start: try c.decode(Int.self, forKey: .start),
                items: try c.decode([[DocBlock]].self, forKey: .items))
        case "quote":
            self = .quote(blocks: try c.decode([DocBlock].self, forKey: .blocks))
        case "preformatted":
            self = .preformatted(lines: try c.decode([String].self, forKey: .lines))
        case "table":
            self = .table(rows: try c.decode([[DocCell]].self, forKey: .rows))
        case "rule":
            self = .rule
        case "fold":
            self = .fold(
                label: try c.decode(String.self, forKey: .label),
                expanded: try c.decode(Bool.self, forKey: .expanded),
                toggle: try c.decode(ActionId.self, forKey: .toggle),
                blocks: try c.decode([DocBlock].self, forKey: .blocks))
        case let other:
            throw DecodingError.dataCorruptedError(
                forKey: .type, in: c, debugDescription: "unknown block type \(other)")
        }
    }

    func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .paragraph(spans):
            try c.encode("paragraph", forKey: .type)
            try c.encode(spans, forKey: .spans)
        case let .heading(level, spans):
            try c.encode("heading", forKey: .type)
            try c.encode(level, forKey: .level)
            try c.encode(spans, forKey: .spans)
        case let .list(ordered, start, items):
            try c.encode("list", forKey: .type)
            try c.encode(ordered, forKey: .ordered)
            try c.encode(start, forKey: .start)
            try c.encode(items, forKey: .items)
        case let .quote(blocks):
            try c.encode("quote", forKey: .type)
            try c.encode(blocks, forKey: .blocks)
        case let .preformatted(lines):
            try c.encode("preformatted", forKey: .type)
            try c.encode(lines, forKey: .lines)
        case let .table(rows):
            try c.encode("table", forKey: .type)
            try c.encode(rows, forKey: .rows)
        case .rule:
            try c.encode("rule", forKey: .type)
        case let .fold(label, expanded, toggle, blocks):
            try c.encode("fold", forKey: .type)
            try c.encode(label, forKey: .label)
            try c.encode(expanded, forKey: .expanded)
            try c.encode(toggle, forKey: .toggle)
            try c.encode(blocks, forKey: .blocks)
        }
    }
}

struct DocCell: Codable {
    let header: Bool?
    let spans: [DocSpan]
}

struct DocSpan: Codable {
    let text: String
    let bold: Bool?
    let italic: Bool?
    let code: Bool?
    let strike: Bool?
    let hit: Bool?
    let link: ActionId?
    let image: InlineImage?
}

/// A tapped link's full target, before it opens.
struct LinkSheet: Codable {
    let url: String
    let open: ActionButton
    let copy: ActionButton
    let close: ActionButton
}

struct Attachment: Codable {
    let part: Int
    let name: String
    let mime: String
    let size: String
    let open: ActionId
    let save: ActionId
}

struct SettingsPage: Codable {
    let section: String
    let tabs: [Choice]
    let groups: [SettingsGroup]
}

struct SettingsGroup: Codable {
    let title: String
    let hint: String?
    let rows: [SettingRow]
}

struct SettingRow: Codable {
    let label: String
    let value: String
    let hint: String?
    let choices: [Choice]
    let actions: [ActionButton]
}

/// A text field of the composer, the palette or the account setup (`app_mobile::Field`).
enum Field: String, Codable {
    case to, cc, bcc, subject, body, query, email, name, imap, smtp, password
}

/// The kind of a setup field (`app_mobile::frame::InputKind`): keyboard and autofill.
enum InputKind: String, Codable { case email, name, server, password }

struct SetupField: Codable {
    let field: Field
    let label: String
    let hint: String
    let input: InputKind
    let value: TextValue
}

struct PresetChoice: Codable {
    let label: String
    let hint: String
    let selected: Bool
    let action: ActionId
}

struct SetupSheet: Codable {
    let title: String
    let step: String
    let presets: [PresetChoice]
    let fields: [SetupField]
    let note: String?
    let error: String?
    let busy: Bool
    let actions: [ActionButton]
    let back: ActionButton
}

struct TextValue: Codable {
    let text: String
    let version: Int64
}

struct Chip: Codable {
    let label: String
    let remove: ActionId
}

struct Recipients: Codable {
    let field: Field
    let label: String
    let chips: [Chip]
    let input: TextValue
    let suggestions: [ActionButton]
}

struct Choice: Codable {
    let label: String
    let selected: Bool
    let action: ActionId
}

struct QuoteFold: Codable {
    let label: String
    let header: [String]
    let expanded: Bool
    let toggle: ActionId
}

struct Composer: Codable {
    let draft: String
    let title: String
    let status: String
    let editable: Bool
    let from: String
    let fromChoices: [Choice]
    let recipients: [Recipients]
    let subject: TextValue
    let body: TextValue
    let quote: QuoteFold?
    let attachments: [OutgoingFile]
    let problems: [String]
    let actions: [ActionButton]

    enum CodingKeys: String, CodingKey {
        case draft, title, status, editable, from, recipients, subject, body, quote, attachments, problems, actions
        case fromChoices = "from_choices"
    }
}

struct OutgoingFile: Codable {
    let name: String
    let detail: String
    let failed: Bool
    let remove: ActionId?
}

struct Banner: Codable {
    let text: String
    let hint: String?
    let action: ActionButton?
    let dismiss: ActionButton?
    let info: Bool?
}

struct Snackbar: Codable {
    let text: String
    let action: ActionButton?
}

/// What only the host can do (`app_mobile::External`).
struct External: Codable {
    let external: String
    let args: ExternalArgs?
}

/// The arguments of an `External`; each external fills the ones it has.
struct ExternalArgs: Codable {
    let draft: String?
    let uris: [String]?
    let url: String?
    let text: String?
    let blob: String?
    let files: [SaveFile]?
    let sync: Int?
    let intervalMins: Int?
    let screen: String?

    enum CodingKeys: String, CodingKey {
        case draft, uris, url, text, blob, files, sync, screen
        case intervalMins = "interval_mins"
    }
}

/// One attachment to save; the name was made safe by the host.
struct SaveFile: Codable {
    let part: Int
    let name: String
    let mime: String
}

/// `app_theme`'s tokens (`app_mobile::theme`): colour roles as `#RRGGBBAA`, styles in points.
struct FrameTheme: Codable {
    let id: String
    let name: String
    let dark: Bool
    let content: [String: String]
    let chrome: [String: String]
    let tags: [String]
    let typeScale: [String: TextStyleToken]
    let metrics: [String: JSON]
    let icons: [String: String]

    enum CodingKeys: String, CodingKey {
        case id, name, dark, content, chrome, tags, metrics, icons
        case typeScale = "type_scale"
    }
}

struct TextStyleToken: Codable {
    let face: String
    let size: Double
    let lineHeight: Double
    let weight: Int
    let tracking: Double

    enum CodingKeys: String, CodingKey {
        case face, size, weight, tracking
        case lineHeight = "line_height"
    }
}

struct About: Codable {
    let title: String
    let text: String
    let licences: [Licence]
}

struct Licence: Codable {
    let name: String
    let text: String
}

struct PaletteSheet: Codable {
    let prompt: String?
    let hint: String
    let query: TextValue
    let sections: [PaletteSection]
    let close: ActionButton
}

struct PaletteSection: Codable {
    let title: String
    let rows: [PaletteRow]
}

struct PaletteRow: Codable {
    let title: String
    let detail: String
    let keys: String?
    let action: ActionId
}

/// A query's result, read-only; email content in it is untrusted data.
struct ResultView: Codable {
    let title: String
    let note: String
    let lines: [String]
    let close: ActionButton
}

/// A notification Rust decided on (`app_mobile::Notification`); `open` and the acts go back verbatim.
struct Note: Codable {
    let tag: String
    let channel: String
    let title: String
    let body: String
    let publicTitle: String
    let open: JSON?
    let actions: [NoteAction]?

    enum CodingKeys: String, CodingKey {
        case tag, channel, title, body, open, actions
        case publicTitle = "public_title"
    }
}

struct NoteAction: Codable {
    let label: String
    let act: JSON
}
