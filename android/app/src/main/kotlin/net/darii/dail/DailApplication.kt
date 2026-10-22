package net.darii.dail

import android.app.Application

/**
 * Holds the one [Host] of the process. It opens on first use, never in [onCreate]: broadcasts
 * start the process too (ADR 0009).
 */
class DailApplication : Application() {
    val host: Host by lazy { Host(this) }
}
