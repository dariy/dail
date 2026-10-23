package net.darii.dail

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder

/**
 * Keeps the process alive from a scheduled send to its result (ADR 0009): Rust's `sends` task
 * waits until no send is scheduled or leaving, at most 3 minutes. `shortService` from Android 14,
 * `dataSync` before. Started when a frame asks for `sending`; a second start while it runs is a
 * no-op.
 */
class SendService : Service() {
    @Volatile private var running = false

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(
        intent: Intent?,
        flags: Int,
        startId: Int,
    ): Int {
        val type =
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
                ServiceInfo.FOREGROUND_SERVICE_TYPE_SHORT_SERVICE
            } else {
                ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC
            }
        startForeground(NOTIFICATION, notification(), type)
        if (!running) {
            running = true
            val host = (application as DailApplication).host
            Thread({
                host.background(SENDS)
                running = false
                stopSelf()
            }, "dail-send").start()
        }
        return START_NOT_STICKY
    }

    /** A shortService that outlives its time is stopped instead of crashing the app (Android 14). */
    @Deprecated("Android 15 calls the two-argument version")
    override fun onTimeout(startId: Int) = stopSelf()

    /** The same from Android 15. */
    override fun onTimeout(
        startId: Int,
        fgsType: Int,
    ) = stopSelf()

    private fun notification(): Notification {
        val manager = getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(
            NotificationChannel(CHANNEL, getString(R.string.channel_sending), NotificationManager.IMPORTANCE_LOW),
        )
        return Notification
            .Builder(this, CHANNEL)
            .setSmallIcon(android.R.drawable.ic_menu_send)
            .setContentTitle(getString(R.string.sending))
            .setOngoing(true)
            .build()
    }

    companion object {
        private const val CHANNEL = "sending"
        private const val NOTIFICATION = 1
        private const val SENDS = "\"sends\""

        /** Starts the service (from the activity, which is in the foreground then). */
        fun start(context: Context) {
            context.startForegroundService(Intent(context, SendService::class.java))
        }
    }
}
