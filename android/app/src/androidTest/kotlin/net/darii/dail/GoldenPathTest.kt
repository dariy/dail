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

/**
 * The golden path over the demo build (STORE-9b), driven like an agent drives the app over adb:
 * the active window's accessibility tree to find what is on screen, `input` to tap and type, and the debug bridge's
 * `state` to check what happened. Open and read a conversation, archive it and undo, compose a
 * message, send it and undo the send. No test dependency beyond the runner.
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

    /** A debug bridge answer: `Result: Bundle[{result=<json>}]`. */
    private fun bridge(method: String): JSONObject =
        JSONObject(
            shell("content call --uri content://$app.debug --method $method")
                .substringAfter("result=")
                .trim()
                .removeSuffix("}]"),
        )

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
        walk(root)
        return out
    }

    /** Waits until [find] matches a node on screen, at most [seconds]. */
    private fun await(
        what: String,
        seconds: Int = 20,
        find: (Node) -> Boolean,
    ): Node {
        val deadline = System.currentTimeMillis() + seconds * 1000L
        while (true) {
            val seen = nodes()
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

    private var taps = 0

    /** Taps [node], and records it: a log line and a screenshot just before, numbered, for golden-path.yml. */
    private fun tap(node: Node) {
        taps += 1
        Log.i(TAG, "tap $taps: $node; snackbar: ${bridge("frame").opt("snackbar")}")
        shell("mkdir -p $EVIDENCE")
        shell("screencap -p $EVIDENCE/tap-${taps.toString().padStart(2, '0')}.png")
        shell("input tap ${node.x} ${node.y}")
    }

    private fun tap(label: String) = tap(await(label, find = labelled(label)))

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
        tap("Archive")
        awaitThat("archived") { !listed(SUBJECT) }
        tap("Undo")
        awaitThat("back in the inbox") { listed(SUBJECT) }

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
        // Send, then undo inside the window.
        tap("Send")
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
        const val TAG = "GoldenPath"
        const val EVIDENCE = "/data/local/tmp/golden"
    }
}
