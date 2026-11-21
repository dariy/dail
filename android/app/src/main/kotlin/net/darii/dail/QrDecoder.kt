package net.darii.dail

import com.google.zxing.BarcodeFormat
import com.google.zxing.BinaryBitmap
import com.google.zxing.DecodeHintType
import com.google.zxing.MultiFormatReader
import com.google.zxing.PlanarYUVLuminanceSource
import com.google.zxing.ReaderException
import com.google.zxing.common.HybridBinarizer

/**
 * Pixels to text for the in-app scanner (SRV-QR3): zxing's pure-Java reader, QR codes only, dark on
 * light or light on dark (a terminal's code). It only decodes: what the text is, and whether it is
 * used, is Rust's (`scanned`). One instance per analysis thread (the reader keeps state).
 */
class QrDecoder {
    private val reader =
        MultiFormatReader().apply {
            setHints(
                mapOf(
                    DecodeHintType.POSSIBLE_FORMATS to listOf(BarcodeFormat.QR_CODE),
                    DecodeHintType.CHARACTER_SET to "UTF-8",
                    DecodeHintType.ALSO_INVERTED to true,
                ),
            )
        }

    /**
     * The text of a QR code in [luminance] (one byte per pixel, as a camera's Y plane: rows
     * [rowStride] bytes apart), or `null` when there is none or the size does not add up.
     */
    fun decode(
        luminance: ByteArray,
        width: Int,
        height: Int,
        rowStride: Int = width,
    ): String? {
        if (width <= 0 || height <= 0 || rowStride < width) return null
        if (luminance.size.toLong() < rowStride.toLong() * (height - 1) + width) return null
        val source = PlanarYUVLuminanceSource(luminance, rowStride, height, 0, 0, width, height, false)
        return try {
            reader.decodeWithState(BinaryBitmap(HybridBinarizer(source))).text
        } catch (_: ReaderException) {
            null
        } finally {
            reader.reset()
        }
    }
}
