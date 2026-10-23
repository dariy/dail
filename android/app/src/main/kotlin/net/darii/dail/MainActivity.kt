package net.darii.dail

import android.content.ActivityNotFoundException
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Intent
import android.content.res.Configuration
import android.net.ConnectivityManager
import android.net.Network
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.DocumentsContract
import android.view.HapticFeedbackConstants
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.core.net.toUri
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.delay
import net.darii.dail.ui.DailScreen
import net.darii.dail.ui.DailTheme
import net.darii.dail.ui.LocalBlobs
import net.darii.dail.ui.SaveFile
import net.darii.dail.ui.UiEvent
import net.darii.dail.ui.blobRequest
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

    /** The attachments the open "create document" or folder picker saves. */
    private var saving: Pair<String, List<SaveFile>>? = null

    /** One file: the system's "create document" picker, which never overwrites without asking. */
    private val createOne =
        registerForActivityResult(ActivityResultContracts.CreateDocument("*/*")) { uri ->
            val (blob, files) = saving ?: return@registerForActivityResult
            saving = null
            val file = files.firstOrNull() ?: return@registerForActivityResult
            if (uri != null) io.execute { report(listOf(write(blob, file, uri))) }
        }

    /** Several files: a folder; each becomes a new document (the provider renames taken names). */
    private val createAll =
        registerForActivityResult(ActivityResultContracts.OpenDocumentTree()) { tree ->
            val (blob, files) = saving ?: return@registerForActivityResult
            saving = null
            if (tree == null) return@registerForActivityResult
            io.execute {
                val folder =
                    DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree))
                report(
                    files.map { file ->
                        val uri =
                            runCatching {
                                DocumentsContract.createDocument(contentResolver, folder, file.mime, file.name)
                            }.getOrNull()
                        uri != null && write(blob, file, uri)
                    },
                )
            }
        }

    private fun save(
        blob: String,
        files: List<SaveFile>,
    ) {
        saving = blob to files
        if (files.size == 1) createOne.launch(files[0].name) else createAll.launch(null)
    }

    /** Writes one attachment's bytes (read through `blob`) into a new document. */
    private fun write(
        blob: String,
        file: SaveFile,
        uri: Uri,
    ): Boolean {
        val bytes = host.blob(blobRequest("attachment", blob, part = file.part)) ?: return false
        return runCatching {
            contentResolver.openOutputStream(uri, "w")?.use { it.write(bytes) } != null
        }.getOrDefault(false)
    }

    private fun report(saved: List<Boolean>) {
        val good = saved.count { it }
        val text =
            when {
                good == saved.size && good == 1 -> "Saved."
                good == saved.size -> "Saved $good files."
                else -> "Saved $good of ${saved.size} files."
            }
        runOnUiThread { Toast.makeText(this, text, Toast.LENGTH_SHORT).show() }
    }

    /** Opens an `http(s)` URL the user saw in full and chose to open, in the browser. */
    private fun openUrl(url: String) {
        val uri = url.toUri()
        if (uri.scheme?.lowercase() !in setOf("http", "https")) return
        val view = Intent(Intent.ACTION_VIEW, uri).addCategory(Intent.CATEGORY_BROWSABLE)
        try {
            startActivity(view)
        } catch (_: ActivityNotFoundException) {
            Toast.makeText(this, "No app can open this link.", Toast.LENGTH_SHORT).show()
        }
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

                "open_url" -> {
                    external.args?.url?.let { url -> runOnUiThread { openUrl(url) } }
                }

                "save_files" -> {
                    val blob = external.args?.blob
                    val files = external.args?.files.orEmpty()
                    if (blob != null && files.isNotEmpty()) runOnUiThread { save(blob, files) }
                }

                "copy" -> {
                    external.args?.text?.let { text ->
                        runOnUiThread {
                            getSystemService(ClipboardManager::class.java)
                                ?.setPrimaryClip(ClipData.newPlainText("link", text))
                        }
                    }
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
            CompositionLocalProvider(LocalBlobs provides host::blob) {
                DailTheme(theme, dark) { DailScreen(screen, host::send) }
            }
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

    /** A network that comes back while the app is visible syncs (the first answer is the current one). */
    private val network =
        object : ConnectivityManager.NetworkCallback() {
            @Volatile var seen = false

            override fun onAvailable(network: Network) {
                if (seen) host.send(UiEvent.Online) else seen = true
            }

            override fun onLost(network: Network) {
                seen = true
            }
        }

    override fun onStart() {
        super.onStart()
        host.send(UiEvent.Visible(true))
        network.seen = false
        getSystemService(ConnectivityManager::class.java)?.registerDefaultNetworkCallback(network)
    }

    override fun onStop() {
        getSystemService(ConnectivityManager::class.java)?.let { runCatching { it.unregisterNetworkCallback(network) } }
        host.send(UiEvent.Visible(false))
        // Queued triage, draft saves and attachment saves still reach the servers.
        DailJobService.flush(this)
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
