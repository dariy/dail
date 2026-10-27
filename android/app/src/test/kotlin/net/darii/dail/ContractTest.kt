package net.darii.dail

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import net.darii.dail.ui.External
import net.darii.dail.ui.Field
import net.darii.dail.ui.Frame
import net.darii.dail.ui.UiEvent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/** The contract fixtures `app-mobile`'s tests write (`crates/app-mobile/tests/it/contract.rs`). */
class ContractTest {
    private val strict = Json { explicitNulls = false }

    private fun fixtures(kind: String): Map<String, String> {
        val dir = File(checkNotNull(javaClass.classLoader?.getResource("contract/$kind")).toURI())
        return dir
            .listFiles()
            .orEmpty()
            .filter { it.extension == "json" }
            .associate { it.nameWithoutExtension to it.readText() }
    }

    @Test
    fun everyFrameDecodesStrictly() {
        val frames = fixtures("frames")
        assertTrue(frames.isNotEmpty())
        frames.forEach { (name, json) ->
            val frame = strict.decodeFromString(Frame.serializer(), json)
            assertTrue(name, frame.revision > 0)
        }
    }

    @Test
    fun everyExternalDecodesStrictly() {
        fixtures("externals").forEach { (name, json) ->
            assertEquals(name, strict.decodeFromString(External.serializer(), json).external)
        }
    }

    @Test
    fun everyNotificationDecodesStrictly() {
        val notes = fixtures("notifications")
        assertEquals(setOf("new_mail", "not_sent", "push"), notes.keys)
        notes.forEach { (name, json) ->
            val note = strict.decodeFromString(Note.serializer(), json)
            assertEquals(name, if (note.tag == "push") "push" else note.channel)
            assertTrue(name, note.publicTitle.isNotEmpty())
        }
    }

    /** Kotlin's events, built as the app builds them, equal Rust's fixtures; a new one fails here. */
    @Test
    fun everyEventEncodesAsRustExpects() {
        val kotlin =
            mapOf(
                "back" to UiEvent.Back,
                "blur" to UiEvent.Blur(Field.To),
                "configure" to
                    UiEvent.Configure(360, 800, dark = true, fontScale = 1.25f, locale = "en-US", keyboard = false),
                "key" to UiEvent.Key("Ctrl+Z"),
                "key_down" to UiEvent.KeyDown(33, 101, ctrl = false, shift = false, alt = false, meta = false),
                "long_press" to UiEvent.LongPress(8),
                "refresh" to UiEvent.Refresh,
                "online" to UiEvent.Online,
                "notification" to
                    UiEvent.Notification(
                        buildJsonObject {
                            put("act", "open_thread")
                            put("thread", "00000000-0000-0000-0000-000000000009")
                        },
                    ),
                "pinch" to UiEvent.Pinch(zoomIn = true),
                "scrolled" to UiEvent.Scrolled(first = 20, count = 12),
                "share" to
                    UiEvent.Share(
                        mailto = "mailto:bob@example.org?subject=Hi",
                        to = listOf("carol@example.org"),
                        subject = "Photos",
                        text = "Here",
                        files = listOf("content://com.example.files/a.pdf"),
                    ),
                "swipe" to UiEvent.Swipe(9),
                "tap" to UiEvent.Tap(7),
                "text" to UiEvent.Text(Field.Subject, "Lunch?"),
                "tick" to UiEvent.Tick,
                "visible" to UiEvent.Visible(true),
            )
        val rust = fixtures("events")
        assertEquals(rust.keys, kotlin.keys)
        rust.forEach { (name, json) ->
            assertEquals(name, strict.parseToJsonElement(json), kotlin.getValue(name).json())
        }
    }
}
