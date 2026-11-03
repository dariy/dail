package net.darii.dail

import android.app.Activity
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.os.Build
import java.io.File
import java.io.RandomAccessFile

/**
 * What the failure screen offers (STORE-5), in Kotlin only: Rust may be dead when it shows.
 * Nothing leaves the device unless the user pastes it somewhere.
 */
object Diagnostics {
    /** The most of `android.log`'s tail that is read, so a huge log never stalls the screen. */
    private const val TAIL_BYTES = 16 * 1024L
    private const val TAIL_LINES = 40

    /** The app's version, the device, the error and the log's last lines, as plain text. */
    fun text(
        context: Context,
        error: String,
    ): String =
        buildString {
            appendLine("dail ${BuildConfig.VERSION_NAME} (${BuildConfig.FLAVOR})")
            appendLine(
                "${Build.MANUFACTURER} ${Build.MODEL}, Android ${Build.VERSION.RELEASE} (API ${Build.VERSION.SDK_INT})",
            )
            appendLine("ABIs: ${Build.SUPPORTED_ABIS.joinToString()}")
            appendLine()
            appendLine(error)
            appendLine()
            append(tail(log(context)))
        }

    /** Copies [text] to the clipboard. */
    fun copy(
        context: Context,
        error: String,
    ) {
        context
            .getSystemService(ClipboardManager::class.java)
            ?.setPrimaryClip(ClipData.newPlainText("dail diagnostics", text(context, error)))
    }

    /** Starts the app afresh in a new process: the only way back once the engine is gone. */
    fun restart(activity: Activity) {
        val launch = activity.packageManager.getLaunchIntentForPackage(activity.packageName) ?: return
        activity.startActivity(Intent.makeRestartActivityTask(launch.component))
        Runtime.getRuntime().exit(0)
    }

    private fun log(context: Context) = context.filesDir.resolve("profiles/default/logs/android.log")

    /** The last lines of [file], read from at most its last [TAIL_BYTES]. */
    internal fun tail(file: File): String =
        runCatching {
            RandomAccessFile(file, "r").use { raf ->
                val start = (raf.length() - TAIL_BYTES).coerceAtLeast(0)
                raf.seek(start)
                val bytes = ByteArray((raf.length() - start).toInt())
                raf.readFully(bytes)
                val lines = String(bytes, Charsets.UTF_8).lines().filter { it.isNotBlank() }
                // A cut first line is dropped unless the whole file was read.
                val whole = if (start > 0) lines.drop(1) else lines
                whole.takeLast(TAIL_LINES).joinToString("\n")
            }
        }.getOrDefault("")
}
