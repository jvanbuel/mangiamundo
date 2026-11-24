package app.mangiamundo.core

import java.time.DateTimeException
import java.time.Duration
import java.time.LocalDate
import java.time.ZonedDateTime

/** One notification to schedule on the device. */
data class PlannedReminder(
    /** Stable key: scheduling the same reminder again replaces it. */
    val key: String,
    val at: ZonedDateTime,
    val title: String,
    val body: String,
    /** What a tap opens; see [ReminderTarget]. */
    val url: String,
)

/**
 * Turns the reminders the server plans (`/api/reminders`) into local notifications. Which
 * reminders there are and what they say is the server's business; the app only schedules.
 */
object ReminderSchedule {
    /** Platforms cap pending notifications (iOS at 64), so only the soonest are scheduled. */
    const val MAX = 48

    /**
     * Each reminder at its hour on its day in the zone of `now`, soonest first. Reminders
     * whose moment has passed, or whose date can't be read, are left out.
     */
    fun requests(reminders: List<Reminder>, now: ZonedDateTime): List<PlannedReminder> =
        reminders.mapNotNull { r ->
            val at = try {
                LocalDate.parse(r.localDate).atTime(r.hour, 0).atZone(now.zone)
            } catch (_: DateTimeException) {
                return@mapNotNull null
            }
            PlannedReminder(r.key, at, r.title, r.body, r.url)
        }.filter { it.at.isAfter(now) }.sortedBy { it.at }.distinctBy { it.key }.take(MAX)

    fun delay(reminder: PlannedReminder, now: ZonedDateTime): Duration = Duration.between(now, reminder.at)
}

/** What a tapped reminder opens: a country, the badges tab, or just the app. */
data class ReminderTarget(val country: String? = null, val badges: Boolean = false) {
    companion object {
        /** Reads the server's link, such as `/?country=JP` or `/?tab=badges`. */
        fun of(url: String?): ReminderTarget {
            val query = url?.substringAfter('?', "")?.substringBefore('#').orEmpty()
            val fields = query.split('&').filter { '=' in it }.associate { it.substringBefore('=') to it.substringAfter('=') }
            return ReminderTarget(fields["country"]?.takeIf { it.isNotEmpty() }, fields["tab"] == "badges")
        }
    }
}
