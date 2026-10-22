package net.darii.dail

import android.content.res.Configuration
import android.os.Build
import android.os.Bundle
import android.view.HapticFeedbackConstants
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.delay
import net.darii.dail.ui.DailScreen
import net.darii.dail.ui.DailTheme
import net.darii.dail.ui.UiEvent
import net.darii.dail.ui.color

/** The one activity: draws the host's frames edge to edge and returns the user's events. */
class MainActivity : ComponentActivity() {
    private val host: Host
        get() = (application as DailApplication).host

    /** A short confirming vibration (`CONFIRM` from Android 11). */
    private val haptic =
        if (Build.VERSION.SDK_INT >=
            Build.VERSION_CODES.R
        ) {
            HapticFeedbackConstants.CONFIRM
        } else {
            HapticFeedbackConstants.KEYBOARD_TAP
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        host.onExternal = { external ->
            if (external.external == "haptic") {
                runOnUiThread { window.decorView.performHapticFeedback(haptic) }
            }
        }
        host.open()
        setContent {
            val configuration = LocalConfiguration.current
            val dark =
                configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK == Configuration.UI_MODE_NIGHT_YES
            val size =
                with(LocalDensity.current) {
                    LocalWindowInfo.current.containerSize.let {
                        it.width.toDp() to
                            it.height.toDp()
                    }
                }
            LaunchedEffect(configuration, size) {
                host.send(
                    UiEvent.Configure(
                        widthDp = size.first.value.toInt(),
                        heightDp = size.second.value.toInt(),
                        dark = dark,
                        fontScale = configuration.fontScale,
                        locale = configuration.locales[0].toLanguageTag(),
                        keyboard = configuration.keyboard == Configuration.KEYBOARD_QWERTY,
                    ),
                )
            }
            val screen by host.frame.collectAsStateWithLifecycle()
            val ticking = (screen as? Screen.Ready)?.frame?.wantsTick == true
            LaunchedEffect(ticking) {
                while (ticking) {
                    delay(TICK_MS)
                    host.send(UiEvent.Tick)
                }
            }
            val theme = (screen as? Screen.Ready)?.theme
            // The status bar sits on the top bar (the chrome group: dark in Dail Light too), the
            // navigation bar on the content.
            val chromeDark = theme?.let { color(it.chrome["base"]).luminance() < HALF } ?: dark
            val contentDark = theme?.dark ?: dark
            LaunchedEffect(chromeDark, contentDark) {
                enableEdgeToEdge(statusBarStyle = bars(chromeDark), navigationBarStyle = bars(contentDark))
            }
            DailTheme(theme, dark) { DailScreen(screen, host::send) }
        }
    }

    private fun bars(dark: Boolean) =
        if (dark) SystemBarStyle.dark(TRANSPARENT) else SystemBarStyle.light(TRANSPARENT, TRANSPARENT)

    override fun onStart() {
        super.onStart()
        host.send(UiEvent.Visible(true))
    }

    override fun onStop() {
        host.send(UiEvent.Visible(false))
        super.onStop()
    }

    private companion object {
        const val TICK_MS = 1_000L
        const val HALF = 0.5f
        const val TRANSPARENT = android.graphics.Color.TRANSPARENT
    }
}
