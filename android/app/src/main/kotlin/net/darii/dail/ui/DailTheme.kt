package net.darii.dail.ui

import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.ExperimentalTextApi
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontVariation
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import net.darii.dail.R

/**
 * `app-theme`'s tokens on Material 3. Every colour is a role the frame names; Kotlin picks the
 * Material slot a role fills and nothing else (ADR 0008, ADR 0009).
 */
private val weights = listOf(400, 500, 600)

/** One variable font at each weight of the type scale (its `wght` axis set per weight). */
@OptIn(ExperimentalTextApi::class)
private fun variable(res: Int) =
    FontFamily(
        weights.map { weight ->
            Font(res, FontWeight(weight), variationSettings = FontVariation.Settings(FontVariation.weight(weight)))
        },
    )

/** Geist, JetBrains Mono and the Material Symbols subset, from `crates/app-theme/assets/fonts`. */
val Sans = variable(R.font.geist)
val Mono = variable(R.font.jetbrains_mono)
val Symbols = FontFamily(Font(R.font.material_symbols))

/** `#RRGGBBAA`. */
fun color(hex: String?): Color {
    val value = hex?.removePrefix("#")?.toLongOrNull(16) ?: return Color.Unspecified
    val argb = (value and 0xFF shl 24) or (value ushr 8)
    return Color(argb.toInt())
}

/** The chrome group (top bar, drawer) and the icon glyphs, for the screens. */
data class Chrome(
    val roles: Map<String, String>,
    val icons: Map<String, String>,
    val tags: List<String> = emptyList(),
) {
    fun role(name: String) = color(roles[name])

    /** An account's colour: the frame's index into the theme's tags. */
    fun tag(index: Int) = color(tags.getOrNull(index))
}

val LocalChrome = staticCompositionLocalOf { Chrome(emptyMap(), emptyMap()) }

private fun scheme(theme: FrameTheme): ColorScheme {
    val c = theme.content

    fun r(name: String) = color(c[name])
    val base = if (theme.dark) darkColorScheme() else lightColorScheme()
    return base.copy(
        primary = r("inverse_bg"),
        onPrimary = r("inverse_text"),
        primaryContainer = r("selected"),
        onPrimaryContainer = r("text_strong"),
        secondary = r("emphasis"),
        onSecondary = r("text_strong"),
        secondaryContainer = r("selected"),
        onSecondaryContainer = r("text_strong"),
        tertiary = r("focus"),
        background = r("canvas"),
        onBackground = r("text"),
        surface = r("base"),
        onSurface = r("text"),
        surfaceVariant = r("raised"),
        onSurfaceVariant = r("text_secondary"),
        surfaceContainerLowest = r("canvas"),
        surfaceContainerLow = r("base"),
        surfaceContainer = r("base"),
        surfaceContainerHigh = r("raised"),
        surfaceContainerHighest = r("raised"),
        inverseSurface = r("inverse_bg"),
        inverseOnSurface = r("inverse_text"),
        outline = r("border_strong"),
        outlineVariant = r("border_subtle"),
        error = r("danger"),
        onError = r("inverse_text"),
        scrim = r("scrim"),
    )
}

private fun style(token: TextStyleToken?): TextStyle =
    token?.let {
        TextStyle(
            fontFamily = if (it.face == "mono") Mono else Sans,
            fontWeight = FontWeight(it.weight),
            fontSize = it.size.sp,
            lineHeight = it.lineHeight.sp,
            letterSpacing = it.tracking.em,
        )
    } ?: TextStyle.Default

private fun typography(theme: FrameTheme): Typography {
    val t = theme.typeScale
    return Typography(
        headlineSmall = style(t["headline_xl"]),
        titleLarge = style(t["headline_lg"]),
        titleMedium = style(t["headline_md"]),
        titleSmall = style(t["headline_md"]),
        bodyLarge = style(t["body_lg"]),
        bodyMedium = style(t["body_md"]),
        bodySmall = style(t["body_sm"]),
        labelLarge = style(t["body_md"]).copy(fontWeight = FontWeight.Medium),
        labelMedium = style(t["label_md"]),
        labelSmall = style(t["label_sm"]),
    )
}

/** Material 3 from the frame's theme; before the first theme, Material's default for [dark]. */
@Composable
fun DailTheme(
    theme: FrameTheme?,
    dark: Boolean,
    content: @Composable () -> Unit,
) {
    if (theme == null) {
        MaterialTheme(colorScheme = if (dark) darkColorScheme() else lightColorScheme(), content = content)
        return
    }
    CompositionLocalProvider(LocalChrome provides Chrome(theme.chrome, theme.icons, theme.tags)) {
        MaterialTheme(colorScheme = scheme(theme), typography = typography(theme), content = content)
    }
}
