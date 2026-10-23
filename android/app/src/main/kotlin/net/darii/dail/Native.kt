package net.darii.dail

import android.content.Context

/**
 * The six calls into `libdail.so` (`crates/app-android/src/jni.rs`), JSON in and out. `open`,
 * `event`, `frame` and `close` run on the "dail-ui" thread, `blob` and `background` on IO threads.
 */
object Native {
    init {
        System.loadLibrary("dail")
    }

    /** `{"handle": n}` or `{"error": …}`. */
    @JvmStatic external fun open(
        config: String,
        listener: Listener,
        context: Context,
    ): String

    /** One `UiEvent`: `{"frame": …, "externals": […]}` or `{"error": …}`. */
    @JvmStatic external fun event(
        handle: Int,
        event: String,
    ): String

    /** After a wake-up: `{"frame": …, "externals": […]}` or `{"error": …}`. */
    @JvmStatic external fun frame(handle: Int): String

    /** An attachment's bytes or a decoded image, `null` if neither can be read. */
    @JvmStatic external fun blob(
        handle: Int,
        request: String,
    ): ByteArray?

    /** Blocks until the task is done or timed out: `{"done": bool}` or `{"error": …}`. */
    @JvmStatic external fun background(
        handle: Int,
        task: String,
    ): String

    /** Shuts the session down. */
    @JvmStatic external fun close(handle: Int)
}

/** Called by Rust from its runtime threads. */
interface Listener {
    /** A new frame is ready: post `frame` to the dail-ui thread. */
    fun wake()

    /** Post a notification (`app_mobile::Notification`, while no activity is visible). */
    fun alert(json: String)
}
