package net.darii.dail

import android.content.Context
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.put
import net.darii.dail.ui.Answer
import net.darii.dail.ui.External
import net.darii.dail.ui.Frame
import net.darii.dail.ui.UiEvent
import net.darii.dail.ui.contract
import java.util.concurrent.Executors

/**
 * The session as Kotlin sees it: every `open`, `event` and `frame` call runs on one thread,
 * "dail-ui", and the frames they return land in [frame] for Compose to draw.
 */
class Host(
    private val context: Context,
) {
    private val ui =
        Executors.newSingleThreadExecutor { runnable -> Thread(runnable, "dail-ui").apply { isDaemon = true } }
    private val state = MutableStateFlow<Screen>(Screen.Opening)
    private var handle = 0

    @Volatile private var lastFrame: String? = null

    /** What to draw. */
    val frame: StateFlow<Screen> = state.asStateFlow()

    /** What the host is asked to do after an event (a vibration, …); set by the activity. */
    @Volatile var onExternal: (External) -> Unit = {}

    private val listener =
        object : Listener {
            override fun wake() = ui.execute { if (handle > 0) take(Native.frame(handle)) }

            override fun alert(json: String) = Unit
        }

    /** Opens the session on dail-ui, once. */
    fun open() =
        ui.execute {
            if (handle > 0) return@execute
            val profile = context.filesDir.resolve("profiles/default").apply { mkdirs() }
            val config =
                buildJsonObject {
                    put("profile_dir", profile.path)
                    put("cache_dir", context.cacheDir.path)
                    put("backend", contract.parseToJsonElement(BuildConfig.BACKEND))
                    put(
                        "lang",
                        context.resources.configuration.locales[0]
                            .toLanguageTag(),
                    )
                    put("version", BuildConfig.VERSION_NAME)
                }
            val answer = contract.parseToJsonElement(Native.open(config.toString(), listener, context)).jsonObject
            val opened = (answer["handle"] as? JsonPrimitive)?.content?.toIntOrNull()
            if (opened == null) {
                state.value = Screen.Failed(error(answer))
            } else {
                handle = opened
                take(Native.frame(handle))
            }
        }

    /**
     * Runs a `background` task on the calling thread (an IO thread: it blocks), opening the
     * session first if no activity has.
     */
    fun background(task: String): String {
        open()
        val opened = ui.submit<Int> { handle }.get()
        if (opened == 0) return """{"error": "the session did not open"}"""
        return Native.background(opened, task)
    }

    /** The JSON of the frame last drawn, for the debug bridge. */
    fun frameJson(): String? = lastFrame

    /** Sends one event on dail-ui. */
    fun send(event: UiEvent) = ui.execute { if (handle > 0) take(Native.event(handle, event.json().toString())) }

    private fun take(json: String) {
        val answer = contract.parseToJsonElement(json).jsonObject
        if ("error" in answer) {
            state.value = Screen.Failed(error(answer))
            return
        }
        val decoded = contract.decodeFromJsonElement(Answer.serializer(), answer)
        lastFrame = answer["frame"].toString()
        state.value = Screen.Ready(decoded.frame)
        decoded.externals.forEach { onExternal(it) }
    }

    private fun error(answer: JsonObject) = (answer["error"] as? JsonPrimitive)?.content ?: answer.toString()
}

/** The activity's state: opening, a frame, or why the session could not open. */
sealed interface Screen {
    data object Opening : Screen

    data class Ready(
        val frame: Frame,
    ) : Screen

    data class Failed(
        val error: String,
    ) : Screen
}
