package net.darii.dail

import android.content.ContentProvider
import android.content.ContentValues
import android.database.Cursor
import android.net.Uri
import android.os.Binder
import android.os.Bundle
import android.os.Process
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import net.darii.dail.ui.contract

/**
 * The debug adb bridge (debug builds only, ADR 0009): `adb shell content call --uri
 * content://net.darii.dail[.demo].debug --method <m> [--arg <json>]`. It answers only the shell UID.
 *
 * - `exec`: the command in `arg`, submitted as `cli` (as the user at a terminal);
 * - `exec_agent`: the same as `mcp`, under the agent policy;
 * - `state`: the whole `AppState`;
 * - `frame`: the frame on screen.
 *
 * The answer is the bundle's `result`, JSON.
 */
class DebugBridge : ContentProvider() {
    override fun onCreate() = true

    override fun call(
        method: String,
        arg: String?,
        extras: Bundle?,
    ): Bundle {
        if (Binder.getCallingUid() !=
            Process.SHELL_UID
        ) {
            throw SecurityException("the debug bridge answers only adb's shell")
        }
        val host = (checkNotNull(context).applicationContext as DailApplication).host
        val result =
            when (method) {
                "exec" -> host.background(exec("cli", arg))
                "exec_agent" -> host.background(exec("mcp", arg))
                "state" -> host.background("\"state\"")
                "frame" -> host.frameJson() ?: """{"error": "no frame yet"}"""
                else -> """{"error": "unknown method ${JsonPrimitive(method)}"}"""
            }
        return Bundle().apply { putString("result", result) }
    }

    private fun exec(
        origin: String,
        command: String?,
    ): String =
        buildJsonObject {
            put(
                "exec",
                buildJsonObject {
                    put("origin", origin)
                    put("command", contract.parseToJsonElement(command ?: "null"))
                },
            )
        }.toString()

    override fun query(
        uri: Uri,
        projection: Array<out String>?,
        selection: String?,
        selectionArgs: Array<out String>?,
        sortOrder: String?,
    ): Cursor? = null

    override fun getType(uri: Uri): String? = null

    override fun insert(
        uri: Uri,
        values: ContentValues?,
    ): Uri? = null

    override fun delete(
        uri: Uri,
        selection: String?,
        selectionArgs: Array<out String>?,
    ) = 0

    override fun update(
        uri: Uri,
        values: ContentValues?,
        selection: String?,
        selectionArgs: Array<out String>?,
    ) = 0
}
