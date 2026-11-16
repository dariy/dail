package net.darii.dail

import android.accessibilityservice.AccessibilityServiceInfo
import android.graphics.Rect
import android.os.ParcelFileDescriptor
import android.util.Log
import android.view.accessibility.AccessibilityNodeInfo
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.json.JSONObject
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TestWatcher
import org.junit.runner.Description
import org.junit.runner.RunWith
import kotlin.math.abs

/**
 * The golden path over the demo build (STORE-9b), driven like an agent drives the app over adb:
 * the active window's accessibility tree to find what is on screen, `input` to tap and type, and the debug bridge's
 * `state` to check what happened. Open and read a conversation, archive it and undo, compose a
 * message, send it and undo the send (STORE-9b); check two threads and move them, make a rule from the open message,
 * and add a rule in the settings and switch it off (QA-1). No test dependency beyond the runner.
 *
 * The tests share the app's process, so its state: each starts from the inbox ([home]) and leaves the demo mail and
 * the rules as it found them.
 */
@RunWith(AndroidJUnit4::class)
class GoldenPathTest {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val app = instrumentation.targetContext.packageName

    /** A node of the window dump: its text or description, and the centre of its bounds. */
    private data class Node(
        val text: String,
        val description: String,
        val className: String,
        val x: Int,
        val y: Int,
    )

    /** Runs [command] without a shell: it is split on spaces, so no argument may hold one. */
    private fun shell(command: String): String {
        val output = instrumentation.uiAutomation.executeShellCommand(command)
        return ParcelFileDescriptor.AutoCloseInputStream(output).bufferedReader().readText()
    }

    /** A debug bridge answer: `Result: Bundle[{result=<json>}]`. [arg] must hold no space. */
    private fun bridge(
        method: String,
        arg: String? = null,
    ): JSONObject =
        JSONObject(
            shell("content call --uri content://$app.debug --method $method" + (arg?.let { " --arg $it" } ?: ""))
                .substringAfter("result=")
                .trim()
                .removeSuffix("}]"),
        )

    /** Runs [command] as the user at a terminal (`cli`) and returns its receipt's `data`. */
    private fun exec(command: JSONObject): JSONObject {
        val answer = bridge("exec", command.toString())
        val receipt = answer.optJSONObject("receipt") ?: error("refused: $command -> $answer")
        return receipt.optJSONObject("data") ?: JSONObject()
    }

    private fun command(
        name: String,
        args: JSONObject? = null,
    ): JSONObject = JSONObject().put("cmd", name).apply { args?.let { put("args", it) } }

    /** The ID of the demo account's mailbox with [role] (`inbox`, `archive`, ...). */
    private fun mailbox(role: String): String {
        val boxes = exec(command("list_mailboxes")).getJSONArray("mailboxes")
        return (0 until boxes.length())
            .map(boxes::getJSONObject)
            .first { it.optString("role") == role }
            .getString("id")
    }

    /** The subjects of the threads shown in [mailbox], from `list_threads`. */
    private fun subjects(mailbox: String): List<String> {
        val threads = exec(command("list_threads", JSONObject().put("mailbox", mailbox))).getJSONArray("threads")
        return (0 until threads.length()).map {
            threads.getJSONObject(it).getJSONObject("untrusted_content").optString("subject")
        }
    }

    /** The live sorting rules (not the tombstones) by ID, from the state. */
    private fun rules(): Map<String, JSONObject> {
        val rules =
            state().optJSONObject("synced")?.optJSONObject("settings")?.optJSONObject("rules") ?: return emptyMap()
        return rules
            .keys()
            .asSequence()
            .mapNotNull { id -> rules.getJSONObject(id).optJSONObject("rule")?.let { id to it } }
            .toMap()
    }

    private fun deleteRule(id: String) {
        exec(command("delete_rule", JSONObject().put("id", id)))
    }

    private fun state(): JSONObject = bridge("state")

    /** Whether the last drawn frame's list shows a conversation with [subject]. */
    private fun listed(subject: String): Boolean {
        val rows = bridge("frame").optJSONObject("list")?.optJSONArray("rows") ?: return false
        return (0 until rows.length()).any { rows.getJSONObject(it).optString("subject") == subject }
    }

    /** Waits until [holds], at most [seconds]. */
    private fun awaitThat(
        what: String,
        seconds: Int = 20,
        holds: () -> Boolean,
    ) {
        val deadline = System.currentTimeMillis() + seconds * 1000L
        while (!holds()) {
            check(System.currentTimeMillis() < deadline) { "never true: $what" }
            Thread.sleep(250)
        }
    }

    /**
     * The nodes of the active window, from the instrumentation's own `UiAutomation`: a shell
     * `uiautomator dump` cannot connect while the instrumentation holds it.
     */
    private fun nodes(): List<Node> {
        val root = instrumentation.uiAutomation.rootInActiveWindow ?: return emptyList()
        return flatten(listOf(root))
    }

    /** The nodes under [roots], in order, each with the centre of its bounds. */
    private fun flatten(roots: List<AccessibilityNodeInfo>): List<Node> {
        val out = mutableListOf<Node>()
        val bounds = Rect()

        fun walk(info: AccessibilityNodeInfo) {
            info.getBoundsInScreen(bounds)
            out +=
                Node(
                    text = info.text?.toString().orEmpty(),
                    description = info.contentDescription?.toString().orEmpty(),
                    className = info.className?.toString().orEmpty(),
                    x = bounds.centerX(),
                    y = bounds.centerY(),
                )
            for (i in 0 until info.childCount) info.getChild(i)?.let(::walk)
        }
        roots.forEach(::walk)
        return out
    }

    /**
     * The nodes of every window of the app, the topmost first: a menu (`DropdownMenu`) or a sheet is a popup window of
     * its own, which need not be the active one.
     */
    private fun allNodes(): List<Node> {
        val roots =
            instrumentation.uiAutomation.windows
                .sortedByDescending { it.layer }
                .mapNotNull { it.root }
                .filter { it.packageName?.toString() == app }
        return flatten(roots).ifEmpty { nodes() }
    }

    /** Waits until [find] matches a node on screen, at most [seconds]; [among] gives the nodes to look at. */
    private fun await(
        what: String,
        seconds: Int = 20,
        among: () -> List<Node> = ::nodes,
        find: (Node) -> Boolean,
    ): Node {
        val deadline = System.currentTimeMillis() + seconds * 1000L
        while (true) {
            val seen = among()
            seen.firstOrNull(find)?.let { return it }
            check(System.currentTimeMillis() < deadline) {
                // What was there instead, so a failure in CI says why.
                val shown = seen.map { listOf(it.text, it.description).filter(String::isNotEmpty).joinToString("|") }
                "not on screen: $what; seen: ${shown.filter(String::isNotEmpty).take(40)}"
            }
            Thread.sleep(250)
        }
    }

    /** Waits until [holds] is true of the engine's state. */
    private fun awaitState(
        what: String,
        seconds: Int = 20,
        holds: (JSONObject) -> Boolean,
    ) {
        val deadline = System.currentTimeMillis() + seconds * 1000L
        while (true) {
            val state = state()
            if (holds(state)) return
            check(System.currentTimeMillis() < deadline) {
                "state never showed: $what; drafts: ${drafts(state)}; snackbar: ${bridge("frame").opt("snackbar")}; " +
                    "undo nodes: ${nodes().filter(labelled("Undo")).map { "${it.className}@${it.x},${it.y}" }}"
            }
            Thread.sleep(250)
        }
    }

    private fun labelled(label: String) =
        { node: Node -> node.text == label || node.description == label || node.description.startsWith("$label,") }

    /**
     * A node whose text is [label]: also a clickable row that merges its texts (a palette row with its detail, a
     * settings entry with its hint), whose text may be its first text or all of them joined.
     */
    private fun titled(label: String) =
        { node: Node ->
            node.description.isEmpty() &&
                (node.text == label || node.text.startsWith("$label, ") || node.text.startsWith("$label\n"))
        }

    /** The thread row of the inbox whose description (the frame's `a11y`) names [subject]. */
    private fun row(subject: String) = { node: Node -> node.description.split(", ").contains(subject) }

    private var taps = 0

    /**
     * Taps [node], and records it: a log line and a screenshot, numbered, for golden-path.yml. The screenshot comes just
     * before the tap, or just after it when [fast]: on a slow emulator `screencap` takes seconds an offer may not have.
     */
    private fun tap(
        node: Node,
        fast: Boolean = false,
    ) {
        taps += 1
        val shot = "screencap -p $EVIDENCE/tap-${taps.toString().padStart(2, '0')}.png"
        if (fast) {
            shell("input tap ${node.x} ${node.y}")
            Log.i(TAG, "tap $taps (fast): $node; snackbar after: ${bridge("frame").opt("snackbar")}")
            shell("mkdir -p $EVIDENCE")
            shell(shot)
            return
        }
        Log.i(TAG, "tap $taps: $node; snackbar: ${bridge("frame").opt("snackbar")}")
        shell("mkdir -p $EVIDENCE")
        shell(shot)
        shell("input tap ${node.x} ${node.y}")
    }

    /** Presses [node] for a second: a long press. */
    private fun longPress(node: Node) {
        taps += 1
        Log.i(TAG, "long press $taps: $node")
        shell("mkdir -p $EVIDENCE")
        shell("screencap -p $EVIDENCE/tap-${taps.toString().padStart(2, '0')}.png")
        shell("input swipe ${node.x} ${node.y} ${node.x} ${node.y} 1000")
    }

    /** The text of the frame's snackbar while it offers an action (an undo), else null. */
    private fun offer(): String? =
        bridge("frame")
            .optJSONObject("snackbar")
            ?.takeIf { it.optJSONObject("action") != null }
            ?.optString("text")

    /**
     * Undoes the archive with the snackbar's Undo until [SUBJECT] is listed again. The offer lasts 6 s (`NOTICE_MS` in
     * app-mobile) and the reader opens the next conversation meanwhile: a tap can come too late or miss. So the tap is
     * fast, on the lowest Undo, and tried again while the same offer stands. An undo that goes through clears the offer,
     * so a second tap never undoes an earlier step.
     */
    private fun undoArchive() = undoUntil("back in the inbox") { listed(SUBJECT) }

    /** Taps the snackbar's Undo, as [undoArchive] does, until [undone]. */
    private fun undoUntil(
        what: String,
        undone: () -> Boolean,
    ) {
        var offered: String? = null
        awaitThat("the undo offer before: $what (it lasts 6 s)") { offer().also { offered = it } != null }
        repeat(UNDO_TRIES) { attempt ->
            val undo =
                nodes().filter(labelled("Undo")).maxByOrNull { it.y }
                    ?: await("the snackbar's Undo", find = labelled("Undo"))
            tap(undo, fast = true)
            val deadline = System.currentTimeMillis() + 5_000L
            while (System.currentTimeMillis() < deadline) {
                if (undone()) return
                Thread.sleep(250)
            }
            val now = offer()
            if (now == null) {
                // The undo went through and the app is slow, or the offer ran out: [undone] says which.
                awaitThat("$what; the offer ended after ${attempt + 1} taps", holds = undone)
                return
            }
            check(now == offered) { "never true: $what; offer before: $offered, now: $now" }
        }
        error("never true: $what after $UNDO_TRIES taps on Undo; offer: ${offer()}")
    }

    private fun tap(label: String) = tap(await(label, find = labelled(label)))

    /** The node labelled [label] once it stays put for half a second: a layout still moving would take a tap elsewhere. */
    private fun settled(label: String): Node = settled(label, labelled(label))

    private fun settled(
        what: String,
        find: (Node) -> Boolean,
    ): Node {
        var last = await(what, find = find)
        while (true) {
            Thread.sleep(500)
            val now = await(what, find = find)
            if (now == last) return now
            last = now
        }
    }

    /**
     * Taps the bar's button [label], or the item [label] of the bar's ⋯ menu (*More actions*) when the bar has no room
     * for it: which one depends on the window's width. With several ⋯ on screen, the one nearest [near] (in height).
     */
    private fun barOrMenu(
        label: String,
        near: Node? = null,
    ) {
        nodes().firstOrNull { it.description == label }?.let {
            tap(settled(label) { node -> node.description == label })
            return
        }
        val menus = nodes().filter { it.description == MORE }
        val more =
            (if (near == null) menus.minByOrNull { it.y } else menus.minByOrNull { abs(it.y - near.y) })
                ?: await("$label, or the ⋯ that holds it", find = { it.description == MORE })
        tap(more)
        tap(await("the menu item $label", among = ::allNodes) { it.text == label && it.description.isEmpty() })
    }

    /** The frame's `palette`, while the palette is open. */
    private fun palette(): JSONObject? = bridge("frame").optJSONObject("palette")

    /** Waits for the palette's step that asks [prompt], then taps its row [label] (below the question). */
    private fun answer(
        prompt: String,
        label: String,
    ) {
        awaitThat("the palette asks: $prompt") { palette()?.optString("prompt") == prompt }
        val question = await("the question $prompt") { it.text == prompt }
        tap(await("the answer $label to $prompt") { titled(label)(it) && it.y > question.y })
    }

    /** Back to the demo inbox's list: the system Back while the app takes it (a step, a sheet, a page), then the inbox. */
    private fun home() {
        repeat(HOME_BACKS) {
            val frame = bridge("frame")
            if (!frame.optBoolean("back")) return@repeat
            val revision = frame.optLong("revision")
            shell("input keyevent KEYCODE_BACK")
            awaitThat("Back drawn") { bridge("frame").optLong("revision") != revision }
        }
        check(!bridge("frame").optBoolean("back")) { "still not home: ${bridge("frame").optJSONObject("top_bar")}" }
        if (bridge("frame").optJSONObject("list")?.optString("title") != INBOX) {
            exec(command("open_mailbox", JSONObject().put("id", mailbox("inbox"))))
        }
        awaitThat("the inbox listed", seconds = 60) { listed(SUBJECT) && listed(SECOND) }
    }

    /** The drafts, compactly, for a failure's message. */
    private fun drafts(state: JSONObject): List<String> {
        val drafts = state.optJSONObject("drafts") ?: return emptyList()
        return drafts
            .keys()
            .asSequence()
            .map(drafts::getJSONObject)
            .map {
                "subject=${it.optString("subject")} to=${it.optJSONArray("to")} send_at=${it.opt("send_at")} " +
                    "error=${it.opt("send_error")}"
            }.toList()
    }

    /** Whether a draft is scheduled to leave. */
    private fun scheduled(state: JSONObject): Boolean {
        val drafts = state.getJSONObject("drafts")
        return drafts
            .keys()
            .asSequence()
            .map(drafts::getJSONObject)
            .any { !it.isNull("send_at") }
    }

    /**
     * On failure, while the screen still shows it: a screenshot (`screencap`, as the shell) and every window's node
     * tree in logcat under `GoldenPath`, not only the active window's. golden-path.yml pulls `/data/local/tmp/golden`.
     */
    @get:Rule
    val evidence =
        object : TestWatcher() {
            override fun failed(
                e: Throwable,
                description: Description,
            ) {
                shell("mkdir -p $EVIDENCE")
                shell("screencap -p $EVIDENCE/${description.methodName}.png")
                dumpWindows()
            }
        }

    private fun dumpWindows() {
        val automation = instrumentation.uiAutomation
        // Without this flag `windows` is empty.
        automation.serviceInfo =
            automation.serviceInfo.apply {
                flags = flags or AccessibilityServiceInfo.FLAG_RETRIEVE_INTERACTIVE_WINDOWS
            }
        val bounds = Rect()
        for (window in automation.windows) {
            window.getBoundsInScreen(bounds)
            Log.i(
                TAG,
                "window id=${window.id} type=${window.type} layer=${window.layer} active=${window.isActive} " +
                    "focused=${window.isFocused} title=${window.title} bounds=$bounds",
            )

            fun walk(
                info: AccessibilityNodeInfo,
                depth: Int,
            ) {
                info.getBoundsInScreen(bounds)
                Log.i(
                    TAG,
                    "  ".repeat(depth + 1) +
                        "${info.className} text=${info.text} desc=${info.contentDescription} " +
                        "clickable=${info.isClickable} visible=${info.isVisibleToUser} bounds=$bounds",
                )
                for (i in 0 until info.childCount) info.getChild(i)?.let { walk(it, depth + 1) }
            }
            window.root?.let { walk(it, 0) }
        }
    }

    @Before
    fun launch() {
        // Android 13+ would ask first; the permission is not what this test is about.
        shell("pm grant $app android.permission.POST_NOTIFICATIONS")
        shell("am start -W -n $app/net.darii.dail.MainActivity")
        // Without this flag `windows` is empty (allNodes, dumpWindows).
        val automation = instrumentation.uiAutomation
        automation.serviceInfo =
            automation.serviceInfo.apply {
                flags = flags or AccessibilityServiceInfo.FLAG_RETRIEVE_INTERACTIVE_WINDOWS
            }
        // The app keeps its state between tests: home first, from wherever the last journey ended (a page, a composer).
        awaitThat(
            "the app's first frame",
            seconds = 60,
        ) { runCatching { bridge("frame").has("layout") }.getOrDefault(false) }
        home()
        await("the demo inbox", seconds = 60) { it.description.contains(SUBJECT) || it.text.contains(SUBJECT) }
    }

    /** QA-1: check two threads by touch and move them to a folder from the bulk bar, then undo the move. */
    @Test
    fun checkTwoThreadsAndMoveThem() {
        val inbox = mailbox("inbox")
        val archive = mailbox("archive")
        check(SUBJECT in subjects(inbox) && SECOND in subjects(inbox)) { "the demo inbox: ${subjects(inbox)}" }

        // A long press checks the first thread; with one checked, a tap checks another.
        longPress(settled("the row $SUBJECT", row(SUBJECT)))
        awaitThat("one checked") { bridge("frame").optJSONObject("bulk")?.optString("title") == "1 selected" }
        tap(await("the row $SECOND", find = row(SECOND)))
        awaitThat("two checked") { bridge("frame").optJSONObject("bulk")?.optString("title") == "2 selected" }
        await("the bulk bar's count") { it.text == "2 selected" }

        // Move… opens the palette at its folder step; a folder moves both.
        barOrMenu("Move…")
        answer("Move to…", ARCHIVE_FOLDER)
        awaitThat("both in $ARCHIVE_FOLDER") { subjects(archive).containsAll(listOf(SUBJECT, SECOND)) }
        val left = subjects(inbox)

        // The snackbar's Undo moves them back. It comes first: the offer lasts 6 s and every look through the bridge
        // takes some of them; once it is gone, Compose sits where Undo was.
        undoUntil("both back in the inbox") { subjects(inbox).containsAll(listOf(SUBJECT, SECOND)) }
        assertTrue("still in the inbox after the move: $left", SUBJECT !in left && SECOND !in left)
        val after = subjects(archive)
        assertTrue("still in $ARCHIVE_FOLDER: $after", SUBJECT !in after && SECOND !in after)
        // The moved threads left the list, so they are no longer checked, nor checked again by the undo.
        awaitThat("the bulk bar gone") { bridge("frame").optJSONObject("bulk") == null }
        assertTrue(palette() == null)
    }

    /** QA-1: open a message, open the palette from it, and make a rule from it; the rule is deleted afterwards. */
    @Test
    fun makeARuleFromTheOpenMessage() {
        rules().keys.forEach(::deleteRule)
        tap(await("the row $SUBJECT", find = row(SUBJECT)))
        awaitThat("the conversation in the reader") {
            bridge("frame").optJSONObject("reader")?.optString("subject") == SUBJECT
        }
        // The reader may still be sliding in: wait for its buttons to rest.
        val reply = settled("Reply")

        // The reader's ⋯ (or its bar, on a wide window) opens the palette over the open message.
        barOrMenu("Search or run a command", near = reply)
        awaitThat("the palette open") { palette() != null }
        val query = await("the palette's query field") { it.className == "android.widget.EditText" }
        tap(query)
        shell("input text Make%sa%srule")
        awaitThat("the query typed") { palette()?.getJSONObject("query")?.optString("text") == "Make a rule" }
        tap(await("the row $MAKE_RULE") { titled(MAKE_RULE)(it) && it.y > query.y })

        // Its steps: the sender's test, pre-filled from the message, then an action harmless here.
        answer("Match mail like this one by…", "Sender")
        answer("Then do what?", "Star")
        awaitThat("the palette closed") { palette() == null }

        val made = rules().entries.filter { (_, rule) -> rule.getJSONObject("when").optString("test") == "from" }
        assertTrue("rules: ${rules()}", made.size == 1)
        val (id, rule) = made.single()
        assertTrue("$rule", rule.getJSONObject("when").optString("contains") == SENDER)
        assertTrue("$rule", rule.getJSONArray("then").getJSONObject(0).optString("action") == "star")
        assertTrue("$rule", rule.optBoolean("enabled"))
        deleteRule(id)
        assertTrue(rules().isEmpty())
    }

    /** QA-1: the settings' root lists Rules; add a rule there, switch it off and delete it, all by touch. */
    @Test
    fun addARuleInTheSettingsAndSwitchItOff() {
        rules().keys.forEach(::deleteRule)
        // The drawer's Settings; on a wide window the drawer is a pane, already on screen.
        if (nodes().none(labelled("Settings"))) tap("Mailboxes")
        tap("Settings")
        awaitThat("the settings open") { bridge("frame").optJSONObject("settings") != null }
        val page = bridge("frame").getJSONObject("settings")
        if (bridge("frame").getJSONObject("layout").optString("class") == "compact") {
            // A phone: the root is a list of sections, Rules among them without scrolling a tab row.
            val sections = page.getJSONArray("sections")
            val labels = (0 until sections.length()).map { sections.getJSONObject(it).optString("label") }
            assertTrue("$page", page.optString("section") == "index" && "Rules" in labels)
        }
        tap(await("Rules in the settings", find = titled("Rules")))
        awaitThat("the Rules section") { bridge("frame").optJSONObject("settings")?.optString("section") == "rules" }

        // Add a rule…: the row's button (its label is drawn above it too), then the palette's steps.
        await(ADD_RULE, find = labelled(ADD_RULE))
        tap(nodes().filter(labelled(ADD_RULE)).maxBy { it.y })
        answer("Match mail by what?", "Has an attachment")
        answer("Then do what?", "Star")
        awaitThat("the palette closed") { palette() == null }
        awaitThat("a rule saved") { rules().size == 1 }
        val (id, rule) = rules().entries.single()
        assertTrue("$rule", rule.getJSONObject("when").optString("test") == "has_attachment")
        assertTrue("$rule", rule.optBoolean("enabled"))

        // Its row on the Rules screen: Off switches it off, Delete deletes it.
        val name = rule.getString("name")
        val label = await("the rule's row $name") { it.text == name }
        tap(await("the rule's Off") { it.text == "Off" && it.y > label.y })
        awaitThat("the rule switched off") { rules()[id]?.optBoolean("enabled") == false }
        val shown = await("the rule's row $name") { it.text == name }
        tap(
            nodes().filter { it.text == "Delete" && it.y > shown.y }.minByOrNull { it.y }
                ?: await("the rule's Delete", find = labelled("Delete")),
        )
        awaitThat("the rule deleted") { rules().isEmpty() }
    }

    @Test
    fun openReadArchiveUndoComposeSendUndo() {
        // Open and read.
        // Compose may merge a row's texts into one node: match on containment.
        tap(await("the demo inbox", seconds = 60) { it.text.contains(SUBJECT) || it.description.contains(SUBJECT) })
        await("the reader's Archive", find = labelled("Archive"))
        // Read: the drawn frame shows the conversation in the reader.
        awaitThat("the conversation in the reader") {
            bridge("frame").optJSONObject("reader")?.optString("subject") == SUBJECT
        }

        // Archive, then undo.
        // The reader may still be sliding in: a tap on a moving button lands elsewhere.
        tap(settled("Archive"))
        awaitThat("archived") { !listed(SUBJECT) }
        undoArchive()

        // Back to the list, then compose.
        shell("input keyevent KEYCODE_BACK")
        tap("Compose")
        val fields = {
            await("the composer") { it.className == "android.widget.EditText" }
            nodes().filter { it.className == "android.widget.EditText" }
        }
        tap(fields()[0])
        // The comma takes the address as a recipient.
        shell("input text carol@example.org,")
        // To, Cc and Bcc come first; Subject and the body are the last two fields.
        fields().let { tap(it[it.size - 2]) }
        shell("input text $NEW_SUBJECT")
        tap(fields().last())
        shell("input text Hello")
        // The keyboard would cover the snackbar's Undo.
        shell("input keyevent KEYCODE_BACK")

        // A long undo window, so that the emulator's slow polls don't outlast it.
        shell(
            "content call --uri content://$app.debug --method exec " +
                """--arg {"cmd":"set_setting","args":{"key":"undo_send_ms","value":60000}}""",
        )
        // Send, then undo inside the window. The composer scrolls as the keyboard leaves and the snackbar takes its place,
        // which can carry Send half under the top bar: scroll back to the top, and tap where Send has come to rest.
        shell("input swipe 160 250 160 550")
        tap(settled("Send"))
        awaitState("the send scheduled") { scheduled(it) }
        awaitState("the subject typed") { s -> drafts(s).any { it.startsWith("subject=$NEW_SUBJECT ") } }
        // The snackbar's Undo (cancel_send) is the lowest "Undo" on screen.
        awaitThat("the sending snackbar") { bridge("frame").optJSONObject("snackbar")?.optJSONObject("action") != null }
        await("the snackbar's Undo", find = labelled("Undo"))
        tap(nodes().filter(labelled("Undo")).maxBy { it.y })
        awaitState("the send cancelled") { s -> !scheduled(s) && s.getJSONObject("drafts").length() > 0 }
        assertTrue(!scheduled(state()))
    }

    private companion object {
        /** The demo fixture's first unread INBOX conversation (`crates/app-testkit/fixtures/basic.json`). */
        const val SUBJECT = "Lunch?"
        const val NEW_SUBJECT = "Golden"

        /** The fixture's other INBOX conversation, from Bob; `media` adds a third. */
        const val SECOND = "Q3 numbers"

        /** The sender of [SUBJECT]. */
        const val SENDER = "carol@example.org"

        /** The fixture's folder with the `archive` role, as the move step lists it. */
        const val ARCHIVE_FOLDER = "Archive"
        const val INBOX = "Inbox"
        const val MORE = "More actions"
        const val MAKE_RULE = "Make a rule from this message"
        const val ADD_RULE = "Add a rule…"

        /** Back presses [home] makes at most. */
        const val HOME_BACKS = 8
        const val TAG = "GoldenPath"
        const val EVIDENCE = "/data/local/tmp/golden"

        /** Taps on the archive's Undo before the test gives up. */
        const val UNDO_TRIES = 3
    }
}
