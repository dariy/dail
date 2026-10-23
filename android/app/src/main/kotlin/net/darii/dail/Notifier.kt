package net.darii.dail

import android.Manifest
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import net.darii.dail.ui.contract

/** `app_mobile::Notification`: what Rust's alert watcher hands to `Listener.alert`. */
@Serializable
data class Note(
    val tag: String,
    val channel: String,
    val title: String,
    val body: String,
    @SerialName("public_title") val publicTitle: String,
    val open: JsonObject? = null,
    val actions: List<NoteAction> = emptyList(),
)

/** A button: [act] goes back to Rust verbatim. */
@Serializable
data class NoteAction(
    val label: String,
    val act: JsonObject,
)

/**
 * Posts the notifications Rust decides on (A5). Channels *New mail*, *Not sent* and
 * *Background*; the lock screen shows only `public_title`. A tap opens [MainActivity] with the
 * notification's `open`; a button runs [NoteReceiver]. Kotlin decides nothing: texts, targets
 * and actions all come from `app_mobile::notify`.
 */
object Notifier {
    const val OPEN = "net.darii.dail.OPEN"
    private const val ACT = "act"
    private const val TAG = "tag"

    /** Creates the channels (idempotent). */
    fun channels(context: Context) {
        val manager = context.getSystemService(NotificationManager::class.java) ?: return
        manager.createNotificationChannels(
            listOf(
                NotificationChannel(
                    "new_mail",
                    context.getString(R.string.channel_new_mail),
                    NotificationManager.IMPORTANCE_DEFAULT,
                ),
                NotificationChannel(
                    "not_sent",
                    context.getString(R.string.channel_not_sent),
                    NotificationManager.IMPORTANCE_HIGH,
                ),
                NotificationChannel(
                    "background",
                    context.getString(R.string.channel_background),
                    NotificationManager.IMPORTANCE_MIN,
                ),
            ),
        )
    }

    /** Whether notifications may be posted (Android 13 asks for `POST_NOTIFICATIONS`). */
    fun allowed(context: Context) =
        Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
            context.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED

    /** Posts [json] (`app_mobile::Notification`); unreadable JSON or no permission posts nothing. */
    fun post(
        context: Context,
        json: String,
    ) {
        if (!allowed(context)) return
        val note = runCatching { contract.decodeFromString(Note.serializer(), json) }.getOrNull() ?: return
        channels(context)
        val id = note.tag.hashCode()
        val public =
            Notification
                .Builder(context, note.channel)
                .setSmallIcon(android.R.drawable.ic_dialog_email)
                .setContentTitle(note.publicTitle)
                .build()
        val builder =
            Notification
                .Builder(context, note.channel)
                .setSmallIcon(android.R.drawable.ic_dialog_email)
                .setContentTitle(note.title)
                .setContentText(note.body)
                .setVisibility(Notification.VISIBILITY_PRIVATE)
                .setPublicVersion(public)
                .setAutoCancel(true)
                .setContentIntent(open(context, id, note.open))
        note.actions.forEachIndexed { index, action ->
            val intent =
                Intent(context, NoteReceiver::class.java)
                    .putExtra(ACT, action.act.toString())
                    .putExtra(TAG, note.tag)
            val pending =
                PendingIntent.getBroadcast(
                    context,
                    id * 8 + index,
                    intent,
                    PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
                )
            builder.addAction(Notification.Action.Builder(null, action.label, pending).build())
        }
        context.getSystemService(NotificationManager::class.java)?.notify(note.tag, 0, builder.build())
    }

    private fun open(
        context: Context,
        id: Int,
        act: JsonObject?,
    ): PendingIntent {
        val intent =
            Intent(context, MainActivity::class.java)
                .addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)
        if (act != null) intent.action = OPEN
        act?.let { intent.putExtra(ACT, it.toString()) }
        return PendingIntent.getActivity(
            context,
            id,
            intent,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
    }

    /** The `open` of a notification tap, if [intent] is one. */
    fun opened(intent: Intent?): JsonObject? {
        if (intent?.action != OPEN) return null
        val json = intent.getStringExtra(ACT) ?: return null
        return runCatching { contract.parseToJsonElement(json) as? JsonObject }.getOrNull()
    }

    /** A button: Rust does what its `act` says, as the user; then the notification goes. */
    class NoteReceiver : BroadcastReceiver() {
        override fun onReceive(
            context: Context,
            intent: Intent,
        ) {
            val act =
                intent.getStringExtra(ACT)?.let {
                    runCatching { contract.parseToJsonElement(it) as? JsonObject }.getOrNull()
                } ?: return
            val tag = intent.getStringExtra(TAG)
            val pending = goAsync()
            val host = (context.applicationContext as DailApplication).host
            Thread({
                try {
                    host.background(buildJsonObject { put("notification", act) }.toString())
                    tag?.let { context.getSystemService(NotificationManager::class.java)?.cancel(it, 0) }
                } finally {
                    pending.finish()
                }
            }, "dail-note").start()
        }
    }
}
