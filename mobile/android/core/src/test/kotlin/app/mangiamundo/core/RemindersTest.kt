package app.mangiamundo.core

import java.time.ZoneId
import java.time.ZonedDateTime
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class RemindersTest {
    private val brussels = ZoneId.of("Europe/Brussels")
    private val now = ZonedDateTime.of(2026, 11, 16, 12, 0, 0, 0, brussels)

    private fun celebration(id: String, starts: String) = Celebration(
        id = id, title = id, description = "", countries = listOf("US"), starts = starts, ends = starts,
        nearby = listOf(NearbyCountry("US", 3)), reminder = Reminder("$id is today", "Celebrate."),
    )

    private fun event(id: String, starts: String, ends: String, progress: Int = 0, nearby: Int = 4) = EventProgress(
        id = id, title = id, description = "Eat at 3 places.", countries = listOf("JP"), starts = starts, ends = ends,
        target = 3, progress = progress, earned = progress >= 3, status = EventStatus.UPCOMING, placesNearby = nearby,
    )

    @Test
    fun schedulesTenInTheMorningLocalTime() {
        val plan = ReminderPlan.plan(listOf(celebration("thanksgiving", "2026-11-26")), emptyList(), now)
        assertEquals(1, plan.size)
        assertEquals(ZonedDateTime.of(2026, 11, 26, 10, 0, 0, 0, brussels), plan[0].at)
        assertEquals("thanksgiving is today", plan[0].title)
        assertEquals("US", plan[0].country)
        assertEquals(9 * 24 + 22L, ReminderPlan.delay(plan[0], now).toHours())
    }

    @Test
    fun skipsWhatAlreadyHappened() {
        // Today's 10:00 has passed at noon, and yesterday is long gone.
        val plan = ReminderPlan.plan(listOf(celebration("a", "2026-11-16"), celebration("b", "2026-11-15")), emptyList(), now)
        assertTrue(plan.isEmpty())
    }

    @Test
    fun eventsGetAStartAndALastDaysReminder() {
        val plan = ReminderPlan.plan(emptyList(), listOf(event("japan", "2026-12-01", "2026-12-31", progress = 1)), now)
        assertEquals(listOf("event-start:japan", "event-last:japan"), plan.map { it.key })
        assertEquals(29, plan[1].at.dayOfMonth)
        assertEquals("2 more places to earn the badge before it ends.", plan[1].body)
        assertTrue(plan.all { it.badges })
    }

    @Test
    fun leavesOutEarnedEventsAndOnesWithNothingNearby() {
        val events = listOf(event("done", "2026-12-01", "2026-12-31", progress = 3), event("far", "2026-12-01", "2026-12-31", nearby = 0))
        assertTrue(ReminderPlan.plan(emptyList(), events, now).isEmpty())
    }

    @Test
    fun keepsTheSoonestFew() {
        val many = (1..40).map { celebration("c$it", now.toLocalDate().plusDays(it.toLong()).toString()) }
        val plan = ReminderPlan.plan(many, emptyList(), now)
        assertEquals(ReminderPlan.MAX, plan.size)
        assertEquals("c1", plan.first().key.split(":")[1])
    }
}
