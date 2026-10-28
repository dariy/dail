package net.darii.dail.ui

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject

/**
 * The screen models of `app_mobile::frame`, by hand. The contract fixtures, decoded strictly,
 * keep them in step with Rust (`FrameContractTest`).
 */
val contract = Json { explicitNulls = false }

typealias ActionId = Int

@Serializable
data class Answer(
    val frame: Frame,
    val externals: List<External> = emptyList(),
)

@Serializable
data class Frame(
    val revision: Long,
    val layout: Layout,
    @SerialName("top_bar") val topBar: TopBar,
    val drawer: Drawer? = null,
    val list: ThreadList? = null,
    val reader: Reader? = null,
    val composer: Composer? = null,
    val settings: SettingsPage? = null,
    val setup: SetupSheet? = null,
    val bulk: Bulk? = null,
    val fab: Button? = null,
    val banner: Banner? = null,
    val snackbar: Snackbar? = null,
    val theme: FrameTheme? = null,
    val about: About? = null,
    val palette: PaletteSheet? = null,
    val result: ResultView? = null,
    val link: LinkSheet? = null,
    val preview: Preview? = null,
    val back: Boolean,
    @SerialName("wants_tick") val wantsTick: Boolean,
)

@Serializable
enum class WindowClass {
    @SerialName("compact")
    Compact,

    @SerialName("medium")
    Medium,

    @SerialName("expanded")
    Expanded,
}

@Serializable
enum class PaneKind {
    @SerialName("drawer")
    Drawer,

    @SerialName("list")
    List,

    @SerialName("reader")
    Reader,

    @SerialName("composer")
    Composer,

    @SerialName("settings")
    Settings,
}

@Serializable
data class Layout(
    @SerialName("class") val windowClass: WindowClass,
    val panes: List<PaneKind>,
    /** A pane drawn as a page over the panes and the top bar, from the right (MISC-18). */
    val page: PaneKind? = null,
)

@Serializable
data class Button(
    val icon: String? = null,
    val label: String,
    val action: ActionId,
)

@Serializable
data class TopBar(
    val title: String,
    val subtitle: String? = null,
    val navigation: Button? = null,
    val actions: List<Button>,
)

@Serializable
data class Drawer(
    val modal: Boolean,
    val rail: Boolean,
    val style: Button? = null,
    @SerialName("all_inboxes") val allInboxes: DrawerItem? = null,
    val accounts: List<DrawerAccount>,
    val footer: List<DrawerItem>,
)

@Serializable
data class DrawerAccount(
    val name: String,
    val email: String,
    val tag: Int? = null,
    val fold: Fold? = null,
    val mailboxes: List<DrawerItem>,
)

@Serializable
data class Fold(
    val collapsed: Boolean,
    val action: ActionId,
    val a11y: String,
)

@Serializable
data class DrawerItem(
    val label: String,
    val depth: Int = 0,
    val fold: Fold? = null,
    val icon: String,
    val unread: Int,
    val selected: Boolean,
    val action: ActionId,
    val a11y: String,
)

@Serializable
enum class Density {
    @SerialName("cozy")
    Cozy,

    @SerialName("compact")
    Compact,

    @SerialName("preview")
    Preview,
}

@Serializable
data class ThreadList(
    val title: String,
    val total: Int,
    val offset: Int,
    val density: Density,
    val checked: Int,
    val refreshing: Boolean,
    val rows: List<Row>,
    val server: String? = null,
    val empty: String? = null,
)

@Serializable
data class Row(
    val thread: String,
    val action: ActionId,
    @SerialName("long_press") val longPress: ActionId,
    @SerialName("swipe_start") val swipeStart: ActionId,
    @SerialName("swipe_end") val swipeEnd: ActionId,
    val sender: String,
    val subject: String,
    val snippet: String,
    val date: String,
    val unread: Boolean,
    val starred: Boolean,
    val pinned: Boolean = false,
    val attachments: Boolean,
    val messages: Int,
    val tag: Int? = null,
    val selected: Boolean,
    val checked: Boolean,
    val marks: RowMarks? = null,
    val a11y: String,
)

/** A piece of text cut where the search words are. */
@Serializable
data class Piece(
    val text: String,
    val hit: Boolean = false,
)

@Serializable
data class RowMarks(
    val subject: List<Piece>,
    val snippet: List<Piece>,
)

@Serializable
data class Bulk(
    val title: String,
    val actions: List<Button>,
    val close: Button,
)

@Serializable
data class Reader(
    val thread: String,
    val subject: String,
    val close: Button,
    val maximize: Button? = null,
    val actions: List<Button>,
    val previous: Button? = null,
    val next: Button? = null,
    val position: String? = null,
    val messages: List<ReaderMessage>,
)

@Serializable
data class ReaderMessage(
    val message: String,
    val from: String,
    val to: String,
    val date: String,
    val expanded: Boolean,
    val toggle: ActionId,
    val snippet: String,
    val body: List<DocBlock>? = null,
    val status: String? = null,
    val attachments: List<Attachment>,
    @SerialName("save_all") val saveAll: Button? = null,
    val images: ImagesBar? = null,
    val links: List<Button>,
)

@Serializable
data class ImagesBar(
    val shown: Boolean,
    val text: String,
    val toggle: Button,
)

/** A picture carried in the message: `blob`'s `inline` request. */
@Serializable
data class InlineImage(
    val blob: String,
    val cid: String,
)

@Serializable
enum class PreviewKind {
    @SerialName("image")
    Image,

    @SerialName("text")
    Text,

    @SerialName("none")
    None,
}

/** A received attachment shown in the app; other apps get it only through Save. */
@Serializable
data class Preview(
    val name: String,
    val detail: String,
    val kind: PreviewKind,
    val blob: String,
    val part: Int,
    val note: String? = null,
    val save: Button,
    val close: Button,
)

/** A block of a message body (`app_mobile::frame::DocBlock`), by its `type`. */
@Serializable
sealed interface DocBlock {
    @Serializable
    @SerialName("paragraph")
    data class Paragraph(
        val spans: List<DocSpan>,
    ) : DocBlock

    @Serializable
    @SerialName("heading")
    data class Heading(
        val level: Int,
        val spans: List<DocSpan>,
    ) : DocBlock

    @Serializable
    @SerialName("list")
    data class Items(
        val ordered: Boolean,
        val start: Int,
        val items: List<List<DocBlock>>,
    ) : DocBlock

    @Serializable
    @SerialName("quote")
    data class Quote(
        val blocks: List<DocBlock>,
    ) : DocBlock

    @Serializable
    @SerialName("preformatted")
    data class Preformatted(
        val lines: List<String>,
    ) : DocBlock

    @Serializable
    @SerialName("table")
    data class Table(
        val rows: List<List<DocCell>>,
    ) : DocBlock

    @Serializable
    @SerialName("rule")
    data object Rule : DocBlock

    @Serializable
    @SerialName("fold")
    data class Fold(
        val label: String,
        val expanded: Boolean,
        val toggle: ActionId,
        val blocks: List<DocBlock>,
    ) : DocBlock
}

@Serializable
data class DocCell(
    val header: Boolean = false,
    val spans: List<DocSpan>,
)

@Serializable
data class DocSpan(
    val text: String,
    val bold: Boolean = false,
    val italic: Boolean = false,
    val code: Boolean = false,
    val strike: Boolean = false,
    val hit: Boolean = false,
    val link: ActionId? = null,
    val image: InlineImage? = null,
)

/** A tapped link's full target, before it opens. */
@Serializable
data class LinkSheet(
    val url: String,
    val open: Button,
    val copy: Button,
    val close: Button,
)

@Serializable
data class Attachment(
    val part: Int,
    val name: String,
    val mime: String,
    val size: String,
    val open: ActionId,
    val save: ActionId,
)

@Serializable
data class SettingsPage(
    val section: String,
    val tabs: List<Choice>,
    val groups: List<SettingsGroup>,
)

@Serializable
data class SettingsGroup(
    val title: String,
    val hint: String? = null,
    val rows: List<SettingRow>,
)

@Serializable
data class SettingRow(
    val label: String,
    val value: String,
    val hint: String? = null,
    val choices: List<Choice>,
    val actions: List<Button>,
)

/** The kind of a setup field (`app_mobile::frame::InputKind`): keyboard and autofill. */
@Serializable
enum class InputKind {
    @SerialName("email")
    Email,

    @SerialName("name")
    Name,

    @SerialName("server")
    Server,

    @SerialName("password")
    Password,
}

@Serializable
data class SetupField(
    val field: Field,
    val label: String,
    val hint: String,
    val input: InputKind,
    val value: TextValue,
)

@Serializable
data class PresetChoice(
    val label: String,
    val hint: String,
    val selected: Boolean,
    val action: ActionId,
)

@Serializable
data class SetupSheet(
    val title: String,
    val step: String,
    val presets: List<PresetChoice>,
    val fields: List<SetupField>,
    val note: String? = null,
    val error: String? = null,
    val busy: Boolean,
    val actions: List<Button>,
    val back: Button,
)

@Serializable
data class TextValue(
    val text: String,
    val version: Long,
)

@Serializable
data class Chip(
    val label: String,
    val remove: ActionId,
)

@Serializable
data class Recipients(
    val field: Field,
    val label: String,
    val chips: List<Chip>,
    val input: TextValue,
    val suggestions: List<Button>,
)

@Serializable
data class Choice(
    val label: String,
    val selected: Boolean,
    val action: ActionId,
)

@Serializable
data class QuoteFold(
    val label: String,
    val header: List<String>,
    val expanded: Boolean,
    val toggle: ActionId,
)

@Serializable
data class Composer(
    val draft: String,
    val title: String,
    val status: String,
    val editable: Boolean,
    val from: String,
    @SerialName("from_choices") val fromChoices: List<Choice>,
    val recipients: List<Recipients>,
    val subject: TextValue,
    val body: TextValue,
    val quote: QuoteFold? = null,
    val attachments: List<OutgoingFile>,
    val problems: List<String>,
    val actions: List<Button>,
)

@Serializable
data class OutgoingFile(
    val name: String,
    val detail: String,
    val failed: Boolean,
    val remove: ActionId? = null,
)

@Serializable
data class Banner(
    val text: String,
    val hint: String? = null,
    val action: Button? = null,
    val dismiss: Button? = null,
    val info: Boolean = false,
)

@Serializable
data class Snackbar(
    val text: String,
    val action: Button? = null,
)

/** What only the host can do (`app_mobile::External`). */
@Serializable
data class External(
    val external: String,
    val args: ExternalArgs? = null,
)

/** The arguments of an [External]; each external fills the ones it has. */
@Serializable
data class ExternalArgs(
    val draft: String? = null,
    val uris: List<String> = emptyList(),
    val url: String? = null,
    val text: String? = null,
    val blob: String? = null,
    val files: List<SaveFile> = emptyList(),
    val sync: Int? = null,
    @SerialName("interval_mins") val intervalMins: Int? = null,
    val screen: String? = null,
)

/** One attachment to save; the name was made safe by the host. */
@Serializable
data class SaveFile(
    val part: Int,
    val name: String,
    val mime: String,
)

/** `app_theme`'s tokens (`app_mobile::theme`): colour roles as `#RRGGBBAA`, styles in sp. */
@Serializable
data class FrameTheme(
    val id: String,
    val name: String,
    val dark: Boolean,
    val content: Map<String, String>,
    val chrome: Map<String, String>,
    val tags: List<String>,
    @SerialName("type_scale") val typeScale: Map<String, TextStyleToken>,
    val metrics: JsonObject,
    val icons: Map<String, String>,
)

@Serializable
data class TextStyleToken(
    val face: String,
    val size: Float,
    @SerialName("line_height") val lineHeight: Float,
    val weight: Int,
    val tracking: Float,
)

@Serializable
data class About(
    val title: String,
    val text: String,
    val licences: List<Licence>,
)

@Serializable
data class Licence(
    val name: String,
    val text: String,
)

@Serializable
data class PaletteSheet(
    val prompt: String? = null,
    val hint: String,
    val query: TextValue,
    val sections: List<PaletteSection>,
    val close: Button,
)

@Serializable
data class PaletteSection(
    val title: String,
    val rows: List<PaletteRow>,
)

@Serializable
data class PaletteRow(
    val title: String,
    val detail: String,
    val keys: String? = null,
    val action: ActionId,
)

/**
 * A query's result, read-only: a view (`sections`), or its raw JSON (`lines`) after *Raw data*
 * (MISC-22). Email content in it is untrusted data.
 */
@Serializable
data class ResultView(
    val title: String,
    val note: String,
    val sections: List<ResultSection> = emptyList(),
    val lines: List<String> = emptyList(),
    val raw: Button,
    val close: Button,
)

@Serializable
data class ResultSection(
    val title: String? = null,
    val fields: List<ResultField> = emptyList(),
    val rows: List<ResultRow> = emptyList(),
)

@Serializable
data class ResultField(
    val label: String,
    val value: String,
)

/** A result's row; a tap on one with an `action` opens its thread or mailbox. */
@Serializable
data class ResultRow(
    val title: String,
    val detail: String? = null,
    val meta: String? = null,
    val marks: List<String> = emptyList(),
    val action: Int? = null,
)
