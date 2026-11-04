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
 * no type before (STORE-14). Started when a frame asks for `sending`; a second start while it runs is a
 * no-op.
 */
class SendService : Service() {
    @Volatile private var running = false

    override fun onBind(intent: Intent?): IBinder? = null

    /**
     * `startForeground` with this service's type from Android 14, and without one before: no `dataSync`, so one Play
     * declaration fewer (STORE-14).
     */
    private fun goForeground(note: Notification) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startForeground(NOTIFICATION, note, ServiceInfo.FOREGROUND_SERVICE_TYPE_SHORT_SERVICE)
        } else {
            startForeground(NOTIFICATION, note)
        }
    }

    override fun onStartCommand(
        intent: Intent?,
        flags: Int,
        startId: Int,
    ): Int {
        // Android 12+ may refuse the start (STORE-3): the `sends` job sends instead, without a crash.
        if (runCatching { goForeground(notification()) }.isFailure) {
            DailJobService.sends(this)
            stopSelf()
            return START_NOT_STICKY
        }
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
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle(getString(R.string.sending))
            .setOngoing(true)
            .build()
    }

    companion object {
        private const val CHANNEL = "sending"
        private const val NOTIFICATION = 1
        private const val SENDS = "\"sends\""

        /**
         * Starts the service (from the activity, which is in the foreground then). If the system
         * refuses the start anyway, the `sends` job sends instead (STORE-3).
         */
        fun start(context: Context) {
            runCatching { context.startForegroundService(Intent(context, SendService::class.java)) }
                .onFailure { DailJobService.sends(context) }
        }
    }
}
