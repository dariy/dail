package net.darii.dail

import android.os.ParcelFileDescriptor
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The debug bridge as adb meets it: through `content call` in the shell (UID 2000), over the
 * demo flavour (`connectedDemoDebugAndroidTest`).
 */
@RunWith(AndroidJUnit4::class)
class DebugBridgeTest {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val authority = "${instrumentation.targetContext.packageName}.debug"

    /** `executeShellCommand` splits on spaces and runs no shell: `arg` must hold no space. */
    private fun call(
        method: String,
        arg: String? = null,
    ): String {
        val command = "content call --uri content://$authority --method $method" + (arg?.let { " --arg $it" } ?: "")
        val output = instrumentation.uiAutomation.executeShellCommand(command)
        return ParcelFileDescriptor.AutoCloseInputStream(output).bufferedReader().readText()
    }

    @Test
    fun execRunsACommandAsTheUser() {
        val answer = call("exec", """{"cmd":"list_mailboxes"}""")
        assertTrue(answer, answer.contains("\"ok\":true"))
    }

    @Test
    fun execAgentMeetsTheAgentPolicy() {
        val add = """{"cmd":"add_account","args":{"preset":"hostinger","email":"eve@example.com"}}"""
        val answer = call("exec_agent", add)
        assertTrue(answer, answer.contains("origin_not_allowed"))
    }

    @Test
    fun stateIsJson() {
        val answer = call("state")
        assertTrue(answer, answer.contains("\"mailboxes\""))
    }

    /** The app's own UID (not the shell) is refused. */
    @Test
    fun onlyTheShellIsAnswered() {
        val resolver = instrumentation.targetContext.contentResolver
        val refused =
            runCatching { resolver.call(android.net.Uri.parse("content://$authority"), "state", null, null) }
        assertTrue(refused.exceptionOrNull() is SecurityException)
    }
}
