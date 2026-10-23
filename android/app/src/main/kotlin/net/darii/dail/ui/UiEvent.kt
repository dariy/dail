package net.darii.dail.ui

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.add
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import kotlinx.serialization.json.putJsonArray
import kotlinx.serialization.json.putJsonObject

/** A text field of the composer (`app_mobile::Field`). */
@Serializable
enum class Field(
    val wire: String,
) {
    @SerialName("to")
    To("to"),

    @SerialName("cc")
    Cc("cc"),

    @SerialName("bcc")
    Bcc("bcc"),

    @SerialName("subject")
    Subject("subject"),

    @SerialName("body")
    Body("body"),

    @SerialName("query")
    Query("query"),
}

/**
 * One event for the presenter (`app_mobile::UiEvent`): `{"event": …, "args": {…}}`. Kotlin
 * never builds a command; it returns the action IDs of the frame it shows.
 */
sealed interface UiEvent {
    fun json(): JsonObject

    data class Configure(
        val widthDp: Int,
        val heightDp: Int,
        val dark: Boolean,
        val fontScale: Float,
        val locale: String?,
        val keyboard: Boolean,
    ) : UiEvent {
        override fun json() =
            event("configure") {
                put("width_dp", widthDp)
                put("height_dp", heightDp)
                put("dark", dark)
                put("font_scale", fontScale)
                locale?.let { put("locale", it) }
                put("keyboard", keyboard)
            }
    }

    data class Visible(
        val visible: Boolean,
    ) : UiEvent {
        override fun json() = event("visible") { put("visible", visible) }
    }

    data class Tap(
        val action: ActionId,
    ) : UiEvent {
        override fun json() = event("tap") { put("action", action) }
    }

    data class LongPress(
        val action: ActionId,
    ) : UiEvent {
        override fun json() = event("long_press") { put("action", action) }
    }

    data class Swipe(
        val action: ActionId,
    ) : UiEvent {
        override fun json() = event("swipe") { put("action", action) }
    }

    data class Scrolled(
        val first: Int,
        val count: Int,
    ) : UiEvent {
        override fun json() =
            event("scrolled") {
                put("first", first)
                put("count", count)
            }
    }

    data object Back : UiEvent {
        override fun json() = event("back")
    }

    data object Refresh : UiEvent {
        override fun json() = event("refresh")
    }

    data class Text(
        val field: Field,
        val text: String,
    ) : UiEvent {
        override fun json() =
            event("text") {
                put("field", field.wire)
                put("text", text)
            }
    }

    data class Blur(
        val field: Field,
    ) : UiEvent {
        override fun json() = event("blur") { put("field", field.wire) }
    }

    data class Key(
        val chord: String,
    ) : UiEvent {
        override fun json() = event("key") { put("chord", chord) }
    }

    data object Tick : UiEvent {
        override fun json() = event("tick")
    }

    /** A `mailto:` link or a share from another app; Rust decides what the draft gets. */
    data class Share(
        val mailto: String?,
        val to: List<String>,
        val subject: String?,
        val text: String?,
        val files: List<String>,
    ) : UiEvent {
        override fun json() =
            event("share") {
                mailto?.let { put("mailto", it) }
                if (to.isNotEmpty()) putJsonArray("to") { to.forEach { add(it) } }
                subject?.let { put("subject", it) }
                text?.let { put("text", it) }
                if (files.isNotEmpty()) putJsonArray("files") { files.forEach { add(it) } }
            }
    }
}

private fun event(
    name: String,
    args: (kotlinx.serialization.json.JsonObjectBuilder.() -> Unit)? = null,
) = buildJsonObject {
    put("event", name)
    if (args != null) putJsonObject("args", args)
}
