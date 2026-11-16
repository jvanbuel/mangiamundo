package app.mangiamundo.core

import java.time.Duration
import java.time.LocalDate
import java.time.LocalTime
import java.time.ZonedDateTime

/** One notification to schedule on the device. */
data class PlannedReminder(
    /** Stable key: scheduling the same reminder again replaces it. */
    val key: String,
    val at: ZonedDateTime,
    val title: String,
    val body: String,
    /** Country to open on tap, if any. */
    val country: String?,
    /** Open the badges tab on tap. */
    val badges: Boolean = false,
)

/**
 * Turns the calendar into local notifications: one at 10:00 on the first day of each
 * celebration and event, and a last-days nudge for events the user has not finished.
 * Reminders whose moment has passed are left out.
 */
object ReminderPlan {
    val TIME: LocalTime = LocalTime.of(10, 0)
    const val LAST_CHANCE_DAYS = 3L
    /** Platforms cap pending notifications (iOS at 64), so keep the plan short. */
    const val MAX = 20

    fun plan(celebrations: List<Celebration>, events: List<EventProgress>, now: ZonedDateTime): List<PlannedReminder> {
        fun at(day: String) = LocalDate.parse(day).atTime(TIME).atZone(now.zone)
        val out = mutableListOf<PlannedReminder>()
        for (c in celebrations) {
            out += PlannedReminder("celebration:${c.id}:${c.starts}", at(c.starts), c.reminder.title, c.reminder.body, c.nearby.firstOrNull()?.code)
        }
        for (e in events) {
            if (e.earned || e.placesNearby == 0) continue
            out += PlannedReminder(
                "event-start:${e.id}", at(e.starts), "${e.title} starts today",
                "${e.description} There ${if (e.placesNearby == 1) "is 1 place" else "are ${e.placesNearby} places"} nearby.",
                country = null, badges = true,
            )
            val left = e.target - e.progress
            val lastChance = LocalDate.parse(e.ends).minusDays(LAST_CHANCE_DAYS - 1).toString()
            out += PlannedReminder(
                "event-last:${e.id}", at(lastChance), "Last days of ${e.title}",
                "${if (left == 1) "1 more place" else "$left more places"} to earn the badge before it ends.",
                country = null, badges = true,
            )
        }
        return out.filter { it.at.isAfter(now) }.sortedBy { it.at }.take(MAX)
    }

    fun delay(reminder: PlannedReminder, now: ZonedDateTime): Duration = Duration.between(now, reminder.at)
}
