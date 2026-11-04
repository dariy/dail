package net.darii.dail

import org.junit.Assert.assertEquals
import org.junit.Test
import java.io.File

class DiagnosticsTest {
    @Test
    fun aMissingLogIsEmpty() {
        assertEquals("", Diagnostics.tail(File("/nonexistent/android.log")))
    }

    @Test
    fun aShortLogIsWhole() {
        val file = File.createTempFile("android", ".log").apply { writeText("one\ntwo\n\nthree\n") }
        assertEquals("one\ntwo\nthree", Diagnostics.tail(file))
        file.delete()
    }

    @Test
    fun aLongLogKeepsItsLastFortyWholeLines() {
        val file = File.createTempFile("android", ".log")
        file.writeText((1..5000).joinToString("\n") { "line $it" })
        val lines = Diagnostics.tail(file).lines()
        assertEquals(40, lines.size)
        assertEquals("line 5000", lines.last())
        assertEquals("line 4961", lines.first())
        file.delete()
    }
}
