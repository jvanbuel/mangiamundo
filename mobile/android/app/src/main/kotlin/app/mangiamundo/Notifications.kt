package app.mangiamundo

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import androidx.work.ExistingWorkPolicy
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.Worker
import androidx.work.WorkerParameters
import androidx.work.workDataOf
import app.mangiamundo.core.PlannedReminder
import app.mangiamundo.core.ReminderPlan
import java.time.ZonedDateTime
import java.util.concurrent.TimeUnit

/**
 * Celebration and event reminders as local notifications. The dates are known in
 * advance, so the app schedules them itself: no push service or Firebase project needed.
 * WorkManager keeps them across reboots.
 */
object Notifications {
    private const val TAG = "reminder"
    private const val CHANNEL = "celebrations"
    const val EXTRA_COUNTRY = "country"
    const val EXTRA_BADGES = "badges"

    fun allowed(context: Context): Boolean =
        Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
            ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED

    /** Replaces everything scheduled before with `plan`. */
    fun schedule(context: Context, plan: List<PlannedReminder>, now: ZonedDateTime = ZonedDateTime.now()) {
        val work = WorkManager.getInstance(context)
        work.cancelAllWorkByTag(TAG)
        for (reminder in plan) {
            val request = OneTimeWorkRequestBuilder<ReminderWorker>()
                .setInitialDelay(ReminderPlan.delay(reminder, now).toMillis(), TimeUnit.MILLISECONDS)
                .setInputData(
                    workDataOf(
                        "title" to reminder.title,
                        "body" to reminder.body,
                        EXTRA_COUNTRY to reminder.country,
                        EXTRA_BADGES to reminder.badges,
                    ),
                )
                .addTag(TAG)
                .build()
            work.enqueueUniqueWork(reminder.key, ExistingWorkPolicy.REPLACE, request)
        }
    }

    fun cancelAll(context: Context) {
        WorkManager.getInstance(context).cancelAllWorkByTag(TAG)
    }

    fun show(context: Context, id: Int, title: String, body: String, country: String?, badges: Boolean) {
        if (!allowed(context)) return
        val manager = context.getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(
            NotificationChannel(CHANNEL, "Celebrations", NotificationManager.IMPORTANCE_DEFAULT).apply {
                description = "National days and food festivals of countries with restaurants near you"
            },
        )
        val open = Intent(context, MainActivity::class.java)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
            .putExtra(EXTRA_COUNTRY, country)
            .putExtra(EXTRA_BADGES, badges)
        val tap = PendingIntent.getActivity(context, id, open, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        val notification = NotificationCompat.Builder(context, CHANNEL)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle(title)
            .setContentText(body)
            .setStyle(NotificationCompat.BigTextStyle().bigText(body))
            .setContentIntent(tap)
            .setAutoCancel(true)
            .build()
        try {
            NotificationManagerCompat.from(context).notify(id, notification)
        } catch (_: SecurityException) {
            // Permission was withdrawn after scheduling.
        }
    }
}

class ReminderWorker(context: Context, params: WorkerParameters) : Worker(context, params) {
    override fun doWork(): Result {
        val data = inputData
        Notifications.show(
            applicationContext,
            id.hashCode(),
            data.getString("title") ?: return Result.success(),
            data.getString("body").orEmpty(),
            data.getString(Notifications.EXTRA_COUNTRY),
            data.getBoolean(Notifications.EXTRA_BADGES, false),
        )
        return Result.success()
    }
}
