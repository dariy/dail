package net.darii.dail

import android.app.job.JobInfo
import android.app.job.JobParameters
import android.app.job.JobScheduler
import android.app.job.JobService
import android.content.ComponentName
import android.content.Context

/**
 * Work while the app is off screen (ADR 0009), through JobScheduler: no WorkManager, no Play
 * Services. Both jobs run a `background` task of the host, which opens the session if no
 * activity has.
 * - `flush` (one-off, when the app leaves the screen): queued triage, draft and attachment saves
 *   reach the servers; at most 3 minutes (the host's limit).
 * - `check` (periodic, network required, kept across reboots): `sync_now` as `runtime`, waiting
 *   for the accounts. Scheduled while `background_sync` is 1 or 2 (push keeps it as a fallback),
 *   every `background_interval_mins`; the presenter says so with the `schedule` external.
 */
class DailJobService : JobService() {
    override fun onStartJob(params: JobParameters): Boolean {
        val task =
            when (params.jobId) {
                FLUSH -> "\"flush\""
                CHECK -> "\"check\""
                else -> return false
            }
        val host = (application as DailApplication).host
        Thread({
            host.background(task)
            jobFinished(params, false)
        }, "dail-job").start()
        return true
    }

    /** The system stopped the job (constraints lost, time up): the next run catches up. */
    override fun onStopJob(params: JobParameters): Boolean = params.jobId == FLUSH

    companion object {
        private const val FLUSH = 10
        private const val CHECK = 11
        private const val MINUTE_MS = 60_000L

        private fun scheduler(context: Context) = context.getSystemService(JobScheduler::class.java)

        private fun component(context: Context) = ComponentName(context, DailJobService::class.java)

        /** Flushes queued work soon, with any network (from `onStop`). */
        fun flush(context: Context) {
            scheduler(context)?.schedule(
                JobInfo
                    .Builder(FLUSH, component(context))
                    .setRequiredNetworkType(JobInfo.NETWORK_TYPE_ANY)
                    .build(),
            )
        }

        /**
         * Applies `background_sync` ([sync]: 0 only while open, 1 periodic, 2 push) and the
         * interval: the periodic check runs unless [sync] is 0. Rescheduling the same job with
         * the same interval keeps its timing.
         */
        fun schedule(
            context: Context,
            sync: Int,
            intervalMins: Int,
        ) {
            val scheduler = scheduler(context) ?: return
            if (sync == 0) {
                scheduler.cancel(CHECK)
                return
            }
            val interval = intervalMins.coerceIn(15, 720) * MINUTE_MS
            val pending = scheduler.getPendingJob(CHECK)
            if (pending != null && pending.intervalMillis == interval) return
            scheduler.schedule(
                JobInfo
                    .Builder(CHECK, component(context))
                    .setRequiredNetworkType(JobInfo.NETWORK_TYPE_ANY)
                    .setPersisted(true)
                    .setPeriodic(interval)
                    .build(),
            )
        }
    }
}
