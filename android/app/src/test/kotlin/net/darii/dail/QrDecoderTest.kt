package net.darii.dail

import com.google.zxing.BarcodeFormat
import com.google.zxing.EncodeHintType
import com.google.zxing.qrcode.QRCodeWriter
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import kotlin.random.Random

/** SRV-QR3: the scanner's pixels-to-text step, over luminance built from a QR code zxing writes. */
class QrDecoderTest {
    private val link = "dail://pair?v=1&h=home.example&p=7443&c=ABCD2345&fp=5d41&n=Laptop"

    /** [text] as a QR code in a Y plane: [size]² pixels, rows [stride] bytes apart, padding grey. */
    private fun plane(
        text: String,
        size: Int,
        stride: Int = size,
        inverted: Boolean = false,
    ): ByteArray {
        val matrix = QRCodeWriter().encode(text, BarcodeFormat.QR_CODE, size, size, mapOf(EncodeHintType.MARGIN to 4))
        val dark = if (inverted) LIGHT else DARK
        val light = if (inverted) DARK else LIGHT
        return ByteArray(stride * size) { i ->
            val (x, y) = i % stride to i / stride
            when {
                x >= size -> GREY
                matrix[x, y] -> dark
                else -> light
            }
        }
    }

    @Test
    fun aQrCodeDecodesToItsText() {
        assertEquals(link, QrDecoder().decode(plane(link, 320), 320, 320))
    }

    @Test
    fun rowsWiderThanTheImageAreRead() {
        // A camera's Y plane: each row padded to its stride.
        assertEquals(link, QrDecoder().decode(plane(link, 300, stride = 320), 300, 300, rowStride = 320))
    }

    @Test
    fun lightOnDarkDecodesToo() {
        // A terminal's QR code (`dail pair` in a dark terminal).
        assertEquals(link, QrDecoder().decode(plane(link, 320, inverted = true), 320, 320))
    }

    @Test
    fun anyTextIsReturnedAsItCame() {
        // Which text is a pairing code is Rust's decision.
        assertEquals("https://example.org/a", QrDecoder().decode(plane("https://example.org/a", 240), 240, 240))
    }

    @Test
    fun noQrCodeIsNull() {
        val decoder = QrDecoder()
        assertNull(decoder.decode(ByteArray(320 * 320) { LIGHT }, 320, 320))
        val noise = Random(7)
        assertNull(decoder.decode(ByteArray(320 * 320) { noise.nextInt(256).toByte() }, 320, 320))
        // The same decoder still reads a code afterwards.
        assertEquals(link, decoder.decode(plane(link, 320), 320, 320))
    }

    @Test
    fun sizesThatDoNotAddUpAreNull() {
        val decoder = QrDecoder()
        assertNull(decoder.decode(plane(link, 320), 320, 321))
        assertNull(decoder.decode(plane(link, 320), 320, 320, rowStride = 319))
        assertNull(decoder.decode(ByteArray(0), 0, 0))
    }

    private companion object {
        const val DARK: Byte = 0
        const val LIGHT: Byte = -1
        const val GREY: Byte = 0x7f
    }
}
