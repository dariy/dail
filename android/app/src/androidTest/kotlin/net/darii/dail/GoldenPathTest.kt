package net.darii.dail

import android.graphics.Rect
import android.os.ParcelFileDescriptor
import android.view.accessibility.AccessibilityNodeInfo
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.json.JSONObject
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
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
        while (!holds(state())) {
            check(System.currentTimeMillis() < deadline) { "state never showed: $what" }
            Thread.sleep(250)
        }
    }

    private fun labelled(label: String) =
        { node: Node -> node.text == label || node.description == label || node.description.startsWith("$label,") }

    private fun tap(node: Node) {
        shell("input tap ${node.x} ${node.y}")
    }

    private fun tap(label: String) = tap(await(label, find = labelled(label)))

    /** Whether a draft with [subject] is scheduled to leave. */
    private fun scheduled(
        state: JSONObject,
        subject: String,
    ): Boolean {
        val drafts = state.getJSONObject("drafts")
        return drafts.keys().asSequence().map(drafts::getJSONObject).any {
            it.optString("subject") == subject && !it.isNull("send_at")
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
        tap(fields()[1])
        shell("input text $NEW_SUBJECT")
        tap(fields().last())
        shell("input text Hello")

        // Send, then undo inside the window.
        tap("Send")
        awaitState("the send scheduled") { scheduled(it, NEW_SUBJECT) }
        tap("Undo")
        awaitState("the send cancelled") { s ->
            val drafts = s.getJSONObject("drafts")
            drafts.keys().asSequence().map(drafts::getJSONObject).any {
                it.optString("subject") == NEW_SUBJECT && it.isNull("send_at")
            }
        }
        assertTrue(!scheduled(state(), NEW_SUBJECT))
    }

    private companion object {
        /** The demo fixture's first unread INBOX conversation (`crates/app-testkit/fixtures/basic.json`). */
        const val SUBJECT = "Lunch?"
        const val NEW_SUBJECT = "Golden"
    }
}
