package net.darii.dail

import android.content.Intent
import android.content.res.Configuration
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.view.HapticFeedbackConstants
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
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

    /** The draft the open file picker attaches to. */
    private var picking: String? = null

    /** The system file picker; each picked file is copied and attached off the main thread. */
    private val pick =
        registerForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { uris ->
            val draft = picking ?: return@registerForActivityResult
            picking = null
            if (uris.isEmpty()) return@registerForActivityResult
            io.execute { uris.forEach { host.import(draft, it) } }
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        picking = savedInstanceState?.getString(PICKING)
        host.onExternal = { external ->
            when (external.external) {
                "import_files" -> {
                    external.args?.draft?.let { draft ->
                        val uris = external.args.uris.map(Uri::parse)
                        io.execute { uris.forEach { host.import(draft, it) } }
                    }
                }

                "sending" -> {
                    runOnUiThread { SendService.start(this) }
                }

                "haptic" -> {
                    runOnUiThread { window.decorView.performHapticFeedback(haptic) }
                }

                "pick_files" -> {
                    external.args?.draft?.let { draft ->
                        runOnUiThread {
                            picking = draft
                            pick.launch(arrayOf("*/*"))
                        }
                    }
                }
            }
        }
        host.open()
        if (savedInstanceState == null) shared(intent)?.let(host::send)
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

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        shared(intent)?.let(host::send)
    }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        picking?.let { outState.putString(PICKING, it) }
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
        /** Copies of picked files, one at a time, off the main thread. */
        val io: java.util.concurrent.ExecutorService =
            java.util.concurrent.Executors.newSingleThreadExecutor {
                Thread(
                    it,
                    "dail-import",
                ).apply { isDaemon = true }
            }

        const val TICK_MS = 1_000L
        const val PICKING = "picking"
        const val HALF = 0.5f
        const val TRANSPARENT = android.graphics.Color.TRANSPARENT
    }
}

/**
 * A `mailto:` link or a share as the presenter's `share` event, or `null` for any other intent.
 * Kotlin only reads the extras: which URIs may be attached is Rust's decision.
 */
fun shared(intent: Intent): UiEvent.Share? {
    val action = intent.action
    val mailto = intent.data?.takeIf { it.scheme.equals("mailto", ignoreCase = true) }?.toString()
    if (action !in setOf(Intent.ACTION_SEND, Intent.ACTION_SEND_MULTIPLE) && mailto == null) return null
    val files =
        when (action) {
            Intent.ACTION_SEND -> listOfNotNull(stream(intent))
            Intent.ACTION_SEND_MULTIPLE -> streams(intent)
            else -> emptyList()
        }
    return UiEvent.Share(
        mailto = mailto,
        to = intent.getStringArrayExtra(Intent.EXTRA_EMAIL)?.toList().orEmpty(),
        subject = intent.getStringExtra(Intent.EXTRA_SUBJECT),
        text = intent.getCharSequenceExtra(Intent.EXTRA_TEXT)?.toString(),
        files = files.map(Uri::toString),
    )
}

private fun stream(intent: Intent): Uri? =
    if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.TIRAMISU) {
        intent.getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java)
    } else {
        @Suppress("DEPRECATION")
        intent.getParcelableExtra(Intent.EXTRA_STREAM)
    }

private fun streams(intent: Intent): List<Uri> =
    if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.TIRAMISU) {
        intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM, Uri::class.java).orEmpty()
    } else {
        @Suppress("DEPRECATION")
        intent.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM).orEmpty()
    }
