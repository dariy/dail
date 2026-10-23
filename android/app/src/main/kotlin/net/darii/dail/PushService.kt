package net.darii.dail

import android.app.Notification
import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import androidx.core.content.edit

/**
 * Push (`background_sync` 2, A5): a foreground service that keeps the process, and with it the
 * session and the engine's IMAP IDLE watches, alive. `specialUse` from Android 14, `dataSync`
 * before. Its silent notification is Rust's (`app_mobile::push_status`: *Watching N inboxes*,
 * *Stop*, *Battery*); until the first one arrives it says it is connecting. The periodic check
 * stays scheduled as the fallback. Started and stopped by the `schedule` external, and after a
 * reboot by [Boot] while the last schedule asked for push.
 */
class PushService : Service() {
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(
        intent: Intent?,
        flags: Int,
        startId: Int,
    ): Int {
        val type =
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
                ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE
            } else {
                ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC
            }
        Notifier.channels(this)
        startForeground(NOTIFICATION, starting(), type)
        running = true
        // Rust's notification (with *Stop*) replaces the placeholder: ask for it now that the
        // service can show it. `background` opens the session if no activity has.
        val host = (application as DailApplication).host
        Thread({ host.background(PUSH_TASK) }, "dail-push").start()
        return START_STICKY
    }

    override fun onDestroy() {
        running = false
        super.onDestroy()
    }

    private fun starting(): Notification =
        Notification
            .Builder(this, CHANNEL)
            .setSmallIcon(android.R.drawable.ic_dialog_email)
            .setContentTitle(getString(R.string.push_starting))
            .setOngoing(true)
            .build()

    /** Starts the service after a reboot if the last schedule asked for push. */
    class Boot : BroadcastReceiver() {
        override fun onReceive(
            context: Context,
            intent: Intent,
        ) {
            if (intent.action == Intent.ACTION_BOOT_COMPLETED && wanted(context)) start(context)
        }
    }

    companion object {
        /** The notification's ID, shared with [Notifier] for Rust's updates. */
        const val NOTIFICATION = 2
        const val CHANNEL = "background"
        private const val PUSH = "push"
        private const val PUSH_TASK = "\"push\""

        @Volatile var running = false
            private set

        private fun prefs(context: Context) = context.getSharedPreferences("dail", Context.MODE_PRIVATE)

        private fun wanted(context: Context) = prefs(context).getBoolean(PUSH, false)

        /**
         * Applies `background_sync` ([sync] 2 = push): remembers it for [Boot] and starts or
         * stops the service. A start the system refuses (the app is not in the foreground) waits
         * for the next schedule or reboot.
         */
        fun apply(
            context: Context,
            sync: Int,
        ) {
            val push = sync == 2
            prefs(context).edit { putBoolean(PUSH, push) }
            if (push && !running) {
                start(context)
            } else if (!push && running) {
                context.stopService(Intent(context, PushService::class.java))
            }
        }

        private fun start(context: Context) {
            runCatching { context.startForegroundService(Intent(context, PushService::class.java)) }
        }
    }
}
