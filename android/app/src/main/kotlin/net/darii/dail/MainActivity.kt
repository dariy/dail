package net.darii.dail

import android.content.res.Configuration
import android.os.Bundle
import android.view.HapticFeedbackConstants
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.platform.LocalConfiguration
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.delay
import net.darii.dail.ui.DailScreen
import net.darii.dail.ui.UiEvent

/** The one activity: draws the host's frames edge to edge and returns the user's events. */
class MainActivity : ComponentActivity() {
    private val host: Host
        get() = (application as DailApplication).host

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        host.onExternal = { external ->
            if (external.external == "haptic") {
                runOnUiThread { window.decorView.performHapticFeedback(HapticFeedbackConstants.CONFIRM) }
            }
        }
        host.open()
        setContent {
            val configuration = LocalConfiguration.current
            val dark =
                configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK == Configuration.UI_MODE_NIGHT_YES
            LaunchedEffect(configuration) {
                host.send(
                    UiEvent.Configure(
                        widthDp = configuration.screenWidthDp,
                        heightDp = configuration.screenHeightDp,
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
            MaterialTheme(colorScheme = if (dark) darkColorScheme() else lightColorScheme()) {
                DailScreen(screen, host::send)
            }
        }
    }

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
    }
}
