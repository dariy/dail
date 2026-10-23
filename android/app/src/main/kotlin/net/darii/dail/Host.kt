package net.darii.dail

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.put
import kotlinx.serialization.json.putJsonObject
import net.darii.dail.ui.Answer
import net.darii.dail.ui.External
import net.darii.dail.ui.Frame
import net.darii.dail.ui.FrameTheme
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

    /** The theme of the last frame that carried one (frames carry it only when it changes). */
    private var theme: FrameTheme? = null

    /** What to draw. */
    val frame: StateFlow<Screen> = state.asStateFlow()

    /** What the host is asked to do after an event (a vibration, …); set by the activity. */
    @Volatile var onExternal: (External) -> Unit = {}

    private val listener =
        object : Listener {
            override fun wake() = ui.execute { if (handle > 0) take(Native.frame(handle)) }

            override fun alert(json: String) = Notifier.post(context, json)
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
                    put("outgoing_dir", context.filesDir.resolve("outgoing").path)
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

    /**
     * Copies a picked or shared file into the app and attaches the copy to [draft]: Rust reads it
     * through `/proc/self/fd`, chooses the name and enforces the size limit. Runs on the calling
     * (IO) thread; a failure shows in the snackbar.
     */
    fun import(
        draft: String,
        uri: Uri,
    ) {
        val name =
            context.contentResolver
                .query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
                ?.use { if (it.moveToFirst()) it.getString(0) else null } ?: uri.lastPathSegment.orEmpty()
        val opened = runCatching { context.contentResolver.openFileDescriptor(uri, "r") }.getOrNull() ?: return
        opened.use { fd ->
            val task =
                buildJsonObject {
                    putJsonObject("import") {
                        put("draft", draft)
                        put("name", name)
                        put("source", "/proc/self/fd/${fd.fd}")
                    }
                }
            background(task.toString())
        }
    }

    /**
     * `blob`'s answer to [request] (`{"attachment" | "image" | "text": {blob, part}}` or
     * `{"inline": {blob, cid}}`), on the calling (IO) thread; `null` if it cannot be read.
     */
    fun blob(request: JsonObject): ByteArray? {
        val opened = ui.submit<Int> { handle }.get()
        if (opened == 0) return null
        return Native.blob(opened, request.toString())
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
        decoded.frame.theme?.let { theme = it }
        state.value = Screen.Ready(decoded.frame, theme)
        decoded.externals.forEach { external ->
            // The schedule is the host's, with or without an activity.
            if (external.external == "schedule") {
                val sync = external.args?.sync
                val interval = external.args?.intervalMins
                if (sync != null && interval != null) {
                    DailJobService.schedule(context, sync, interval)
                    PushService.apply(context, sync)
                }
            } else {
                onExternal(external)
            }
        }
    }

    private fun error(answer: JsonObject) = (answer["error"] as? JsonPrimitive)?.content ?: answer.toString()
}

/** The activity's state: opening, a frame, or why the session could not open. */
sealed interface Screen {
    data object Opening : Screen

    data class Ready(
        val frame: Frame,
        val theme: FrameTheme?,
    ) : Screen

    data class Failed(
        val error: String,
    ) : Screen
}
