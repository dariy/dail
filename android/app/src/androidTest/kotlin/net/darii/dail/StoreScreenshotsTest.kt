package net.darii.dail

import android.os.ParcelFileDescriptor
import android.util.Log
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The store listing's screenshots (STORE-18), from the demo build: the same screens in English and in French, written
 * to `/data/local/tmp/store/<lang>/<n>.png` with the names of fastlane's `images/` folders. The screens are reached by
 * commands through the debug bridge, as an agent reaches them, never by taps: what is shot is what the presenter draws
 * after the command. The window's size and density are the workflow's (`store-screenshots.yml`), once for a phone and
 * once for a tablet.
 *
 * Off unless the runner gets `-e store true`, so CI's emulator job does not run it.
 */
@RunWith(AndroidJUnit4::class)
class StoreScreenshotsTest {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val app = instrumentation.targetContext.packageName

    /** Runs [command] without a shell: it is split on spaces, so no argument may hold one. */
    private fun shell(command: String): String {
        val output = instrumentation.uiAutomation.executeShellCommand(command)
        return ParcelFileDescriptor.AutoCloseInputStream(output).bufferedReader().readText()
    }

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

    /** Runs a command as `cli` and returns its receipt's `data`. No string in [args] may hold a space. */
    private fun exec(
        name: String,
        args: JSONObject? = null,
    ): JSONObject {
        val command = JSONObject().put("cmd", name).apply { args?.let { put("args", it) } }
        val answer = bridge("exec", command.toString())
        val receipt = answer.optJSONObject("receipt") ?: error("refused: $command -> $answer")
        return receipt.optJSONObject("data") ?: JSONObject()
    }

    private fun frame(): JSONObject = bridge("frame")

    private fun awaitThat(
        what: String,
        seconds: Int = 30,
        holds: () -> Boolean,
    ) {
        val deadline = System.currentTimeMillis() + seconds * 1000L
        while (!holds()) {
            check(System.currentTimeMillis() < deadline) { "never true: $what" }
            Thread.sleep(250)
        }
    }

    /** The screen once [holds] is true of the drawn frame and the slides have ended, as `<dir>/<n>.png`. */
    private fun shoot(
        dir: String,
        n: Int,
        what: String,
        holds: (JSONObject) -> Boolean,
    ) {
        // A snackbar (the reader's automatic mark-read, say) passes after a few seconds: wait it out.
        awaitThat(what) { frame().let { holds(it) && it.optJSONObject("snackbar") == null } }
        // The frame is drawn; its slide and fade take up to a few hundred milliseconds more.
        Thread.sleep(SETTLE_MS)
        shell("screencap -p $dir/$n.png")
        Log.i(TAG, "shot $dir/$n.png: $what")
    }

    private fun mailbox(role: String): String {
        val boxes = exec("list_mailboxes").getJSONArray("mailboxes")
        return (0 until boxes.length()).map(boxes::getJSONObject).first { it.optString("role") == role }.getString("id")
    }

    private fun thread(
        mailbox: String,
        subject: String,
    ): String {
        val threads = exec("list_threads", JSONObject().put("mailbox", mailbox)).getJSONArray("threads")
        return (0 until threads.length())
            .map(threads::getJSONObject)
            .first { it.getJSONObject("untrusted_content").optString("subject") == subject }
            .getString("id")
    }

    private fun keys(json: JSONObject?): Set<String> =
        json
            ?.keys()
            ?.asSequence()
            ?.toSet()
            .orEmpty()

    /** The five screens, in [language] (1 English, 2 French), into `<STORE>/<folder>`. */
    private fun screens(
        language: Int,
        folder: String,
    ) {
        val dir = "$STORE/$folder"
        shell("mkdir -p $dir")
        exec("set_setting", JSONObject().put("key", "language").put("value", language))
        val inbox = mailbox("inbox")
        exec("open_mailbox", JSONObject().put("id", inbox))

        // 1. The inbox, once drawn in the language: the list's title says which.
        shoot(dir, 1, "the inbox in language $language") { f ->
            val list = f.optJSONObject("list")
            val rows = list?.optJSONArray("rows")?.length() ?: 0
            rows > 0 && (list?.optString("title") == "Inbox") == (language == 1)
        }

        // 2. A conversation in the reader.
        exec("open_thread", JSONObject().put("id", thread(inbox, SUBJECT)))
        shoot(dir, 2, "the reader") { f -> f.optJSONObject("reader")?.optString("subject") == SUBJECT }

        // 3. The palette over the open conversation.
        exec("open_palette")
        shoot(dir, 3, "the palette") { f -> f.optJSONObject("palette") != null }
        exec("close_palette")
        exec("close_thread")

        // 4. The composer with a reply-like draft. Its strings hold no space: the bridge's argument is split on them.
        val state = bridge("state")
        val account = keys(state.optJSONObject("accounts")).first()
        val before = keys(state.optJSONObject("drafts"))
        exec(
            "create_draft",
            JSONObject()
                .put("account", account)
                .put("to", JSONArray().put(JSONObject().put("email", "carol@example.org")))
                .put("subject", "Re: Lunch?")
                .put("body", "Noon works. See you there!"),
        )
        val draft = (keys(bridge("state").optJSONObject("drafts")) - before).single()
        exec("open_composer", JSONObject().put("id", draft))
        shoot(dir, 4, "the composer") { f -> f.optJSONObject("composer") != null }
        exec("close_composer")
        exec("discard_draft", JSONObject().put("id", draft))

        // 5. The settings' appearance.
        exec("open_settings", JSONObject().put("section", "appearance"))
        shoot(dir, 5, "the settings") { f -> f.optJSONObject("settings")?.optString("section") == "appearance" }
        exec("close_settings")
    }

    @Test
    fun storeScreenshots() {
        val args = InstrumentationRegistry.getArguments()
        assumeTrue("only with -e store true", args.getString("store") == "true")
        shell("pm grant $app android.permission.POST_NOTIFICATIONS")
        shell("am start -W -n $app/net.darii.dail.MainActivity")
        awaitThat("the app's first frame", seconds = 60) {
            runCatching { frame().has("layout") }.getOrDefault(false)
        }
        awaitThat("the demo synced", seconds = 60) {
            runCatching { thread(mailbox("inbox"), SUBJECT).isNotEmpty() }.getOrDefault(false)
        }
        shell("rm -rf $STORE")
        try {
            screens(language = 1, folder = "en-US")
            screens(language = 2, folder = "fr-FR")
        } finally {
            exec("set_setting", JSONObject().put("key", "language").put("value", 0))
        }
    }

    private companion object {
        /** The demo fixture's first INBOX conversation (`crates/app-testkit/fixtures/basic.json`). */
        const val SUBJECT = "Lunch?"
        const val STORE = "/data/local/tmp/store"
        const val SETTLE_MS = 1_500L
        const val TAG = "StoreScreenshots"
    }
}
