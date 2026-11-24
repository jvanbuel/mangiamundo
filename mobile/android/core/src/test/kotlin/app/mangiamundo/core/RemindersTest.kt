package app.mangiamundo.core

import java.io.File
import java.time.ZoneId
import java.time.ZonedDateTime
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class RemindersTest {
    private val brussels = ZoneId.of("Europe/Brussels")
    private val now = ZonedDateTime.of(2026, 11, 16, 12, 0, 0, 0, brussels)
    private val json = ApiClient("http://localhost", object : TokenStore { override var token: String? = null }).json

    private fun reminder(key: String, day: String, hour: Int = 10, url: String = "/?tab=badges") =
        Reminder(key = key, localDate = day, hour = hour, title = "$key title", body = "$key body", url = url)

    @Test
    fun decodesWhatTheServerSends() {
        val text = """[{"key":"celebration:burns-night:2027-01-25","local_date":"2027-01-25","hour":10,
            "title":"Burns Night is today","body":"Haggis.","url":"/?country=GB","tag":"celebration-burns-night"}]"""
        val expected = Reminder("celebration:burns-night:2027-01-25", "2027-01-25", 10, "Burns Night is today", "Haggis.", "/?country=GB")
        assertEquals(listOf(expected), json.decodeFromString<List<Reminder>>(text))
    }

    /** A real response, written by the backend's `writes_contract_fixtures` test. */
    @Test
    fun schedulesTheContractFixture() {
        val reminders = json.decodeFromString<List<Reminder>>(File("../../../contract/reminders.json").readText())
        assertTrue(reminders.isNotEmpty())
        assertTrue(reminders.all { it.key.isNotEmpty() && it.title.isNotEmpty() && it.body.isNotEmpty() && it.hour == 10 })
        assertEquals("quests:2026-12", reminders.first().key)
        assertTrue(ReminderTarget.of(reminders.first().url).badges)
        // Every link is one the app can open.
        assertTrue(reminders.all { ReminderTarget.of(it.url) != ReminderTarget() })
        // The fixture is from mid November 2026: seen from then, all of it is still to come.
        val plan = ReminderSchedule.requests(reminders, now)
        assertEquals(reminders.map { it.key }, plan.map { it.key })
        assertEquals(ZonedDateTime.of(2026, 12, 1, 10, 0, 0, 0, brussels), plan.first().at)
    }

    @Test
    fun schedulesAtTheServersHourInLocalTime() {
        val plan = ReminderSchedule.requests(listOf(reminder("a", "2026-11-26", url = "/?country=US")), now)
        assertEquals(listOf(PlannedReminder("a", ZonedDateTime.of(2026, 11, 26, 10, 0, 0, 0, brussels), "a title", "a body", "/?country=US")), plan)
        assertEquals(9 * 24 + 22L, ReminderSchedule.delay(plan[0], now).toHours())
        val evening = ReminderSchedule.requests(listOf(reminder("b", "2026-11-26", hour = 18)), now.withZoneSameInstant(ZoneId.of("Asia/Tokyo")))
        assertEquals(ZonedDateTime.of(2026, 11, 26, 18, 0, 0, 0, ZoneId.of("Asia/Tokyo")), evening[0].at)
    }

    @Test
    fun skipsWhatAlreadyHappenedAndWhatItCannotRead() {
        // Today's 10:00 has passed at noon, and yesterday is long gone; 18:00 today is still ahead.
        val reminders = listOf(
            reminder("a", "2026-11-16"), reminder("b", "2026-11-15"), reminder("c", "2026-11-16", hour = 18),
            reminder("d", "soon"), reminder("e", "2026-11-20", hour = 24),
        )
        assertEquals(listOf("c"), ReminderSchedule.requests(reminders, now).map { it.key })
    }

    @Test
    fun keepsTheSoonestFewInOrder() {
        val many = (40 downTo 1).map { reminder("c$it", now.toLocalDate().plusDays(it.toLong()).toString()) } +
            (1..40).map { reminder("e$it", now.toLocalDate().plusDays(it.toLong()).toString(), hour = 11) }
        val plan = ReminderSchedule.requests(many, now)
        assertEquals(ReminderSchedule.MAX, plan.size)
        assertEquals(listOf("c1", "e1", "c2"), plan.take(3).map { it.key })
        assertEquals("e24", plan.last().key)
    }

    @Test
    fun aKeyIsScheduledOnce() {
        val plan = ReminderSchedule.requests(listOf(reminder("a", "2026-11-26"), reminder("a", "2026-11-27")), now)
        assertEquals(listOf(26), plan.map { it.at.dayOfMonth })
    }

    @Test
    fun readsWhatATapOpens() {
        assertEquals(ReminderTarget(country = "JP"), ReminderTarget.of("/?country=JP"))
        assertEquals(ReminderTarget(badges = true), ReminderTarget.of("/?tab=badges"))
        assertEquals(ReminderTarget("JP", true), ReminderTarget.of("/?tab=badges&country=JP#top"))
        assertEquals(ReminderTarget(), ReminderTarget.of("/"))
        assertEquals(ReminderTarget(), ReminderTarget.of("/?tab=map&country="))
        assertEquals(ReminderTarget(), ReminderTarget.of(null))
    }
}
