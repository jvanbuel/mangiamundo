package app.mangiamundo.core

import java.io.File
import java.time.LocalDate
import java.time.ZoneId
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class CoreTest {
    @Test
    fun visitedTodayFollowsTheLocalDayOfTheLatestVisit() {
        val brussels = ZoneId.of("Europe/Brussels")
        // 2026-11-17 23:30 in Brussels (22:30 UTC), then an older visit.
        val place = Place("p", "P", 0.0, 0.0, distanceM = 0, visits = listOf(Visit(2, 1_794_954_600), Visit(1, 1_794_000_000)))
        assertTrue(place.visitedToday(LocalDate.of(2026, 11, 17), brussels))
        assertFalse(place.visitedToday(LocalDate.of(2026, 11, 18), brussels))
        assertTrue(place.visitedToday(LocalDate.of(2026, 11, 17), ZoneId.of("UTC")))
        assertTrue(place.visitedToday(LocalDate.of(2026, 11, 18), ZoneId.of("Asia/Tokyo")), "07:30 on the 18th there")
        assertFalse(place.copy(visits = emptyList()).visitedToday(LocalDate.of(2026, 11, 17), brussels))
    }

    /** Real responses, written by the backend's `writes_contract_fixtures` test. */
    private val client = ApiClient("http://localhost", object : TokenStore { override var token: String? = null })
    private val json = client.json
    private fun fixture(name: String) = File("../../../contract/$name.json").readText()

    @Test
    fun decodesContractFixtures() {
        assertTrue(json.decodeFromString<Catalog>(fixture("countries")).countries.size > 200)
        val discover = json.decodeFromString<Discover>(fixture("discover"))
        assertEquals(2, discover.placesVisited)
        assertTrue(discover.events.isNotEmpty())
        assertEquals(1, discover.profile.streak)
        assertTrue(discover.quests!!.quests.any { it.id == "check-ins" && it.done })
        assertTrue(discover.badges.any { it.kind == BadgeKind.STREAK })
        val places = json.decodeFromString<List<Place>>(fixture("places"))
        assertEquals(2, places.count { it.visited })
        assertEquals(1, places.count { it.verified })
        val celebrations = json.decodeFromString<List<Celebration>>(fixture("celebrations"))
        assertTrue(celebrations.all { it.nearby.isNotEmpty() })
        assertTrue(places.any { it.hours?.status != null })
        val visit = json.decodeFromString<VisitResult>(fixture("visit"))
        assertTrue(visit.visit.verified)
        assertEquals(1, visit.progress?.placesVisited)
        assertEquals(2, visit.progress?.profile?.level)
        assertEquals(listOf(UnlockedKind.COUNTRY, UnlockedKind.BADGE, UnlockedKind.LEVEL), visit.unlocked.map { it.kind })
        assertEquals("AZ", visit.unlocked[0].country)
        assertEquals(BadgeKind.VISITS, visit.unlocked[1].badge)
        assertEquals(2, visit.unlocked[2].level)
        val catalog = json.decodeFromString<Catalog>(fixture("countries"))
        assertEquals("🇦🇿 Azerbaijan: gold. 2 more for diamond.", visit.unlocked[0].announcement(catalog))
        assertEquals("Level 2. 175 XP so far.", visit.unlocked[2].announcement(catalog))
        val removed = json.decodeFromString<VisitRemoved>(fixture("visit-removed"))
        assertTrue(removed.visits.isEmpty())
        assertEquals(1, removed.progress?.placesVisited)
        assertEquals(1, removed.profile.visits)
        val place = json.decodeFromString<Place>(fixture("place"))
        assertEquals(1, place.visits.size)
        assertTrue(place.verified)
        assertEquals(7, place.hours?.week?.size)
        assertEquals(1, place.hours?.week?.count { it.today })
        assertTrue(places.all { it.hours?.week == null })
        assertTrue(json.decodeFromString<List<Area>>(fixture("areas")).isNotEmpty())
        assertEquals(2, json.decodeFromString<Located>(fixture("located")).options.size)
        assertEquals(96, json.decodeFromString<List<UnknownPlace>>(fixture("unknown-places")).first().distanceM)
        assertEquals("area not found", json.decodeFromString<ErrorBody>(fixture("error")).error)
        val added = client.decodeAdded(fixture("added"))
        assertEquals("Kiwi", added.place.name)
        assertEquals(Reward(380, Bonus.CONTINENT, "Leuven"), added.reward)
        val additions = json.decodeFromString<List<Addition>>(fixture("additions"))
        assertEquals(AdditionStatus.PENDING, additions[0].status)
        assertFalse(additions[0].news)
        assertEquals("user-0123456789abcdef0123456789abcdef", additions[0].id)
        val account = json.decodeFromString<Account>(fixture("account"))
        assertEquals(2, account.devices.size)
        assertEquals("iPhone", account.devices.first { !it.current }.name)
        assertNull(account.devices.first { it.current }.name)
        assertNotNull(account.recoveryCodeCreatedAt)
        assertEquals("ABCD-EFGH", json.decodeFromString<LinkCode>(fixture("link-code")).code)
    }

    /** The answers that are not lists of places: one fixture each. */
    @Test
    fun decodesTheSmallerAnswers() {
        val health = json.decodeFromString<Health>(fixture("health"))
        assertEquals("0123abc", health.commit)
        assertFalse(health.version.isNullOrEmpty())
        assertEquals("0123456789abcdef0123456789abcdef", json.decodeFromString<Session>(fixture("session")).token)
        assertEquals("ABCD-EFGH-JKLM-NPQR-STUV", json.decodeFromString<RecoveryCode>(fixture("recovery-code")).code)
        val link = json.decodeFromString<ShareLink>(fixture("share-link"))
        assertEquals("https://mangiamundo.test/p/abcd2345ef", link.url)
        assertEquals(link.url + "/card.png", link.imageUrl)
        assertTrue("Leuven" in link.text)
        assertEquals(
            AreaProgress(15, "Looking for restaurants"),
            json.decodeFromString<AreaProgress?>(fixture("area-progress")),
        )
    }

    @Test
    fun decodesASuggestionAndWhatARefreshFound() {
        val suggestion = json.decodeFromString<Suggestion>(fixture("suggestion"))
        assertEquals(listOf(suggestion.country), suggestion.place.countries)
        assertTrue(suggestion.place.visits.isEmpty())
        assertEquals(true, suggestion.place.hours?.status?.open)
        val refreshed = json.decodeFromString<RefreshResult>(fixture("refresh"))
        assertEquals(Refreshed.UPDATED, refreshed.status)
        assertEquals("Lalibela", refreshed.place?.name)
        assertEquals(7, refreshed.place?.hours?.week?.size)
        val nearby = json.decodeFromString<NearbyResult>(fixture("nearby"))
        assertEquals(listOf("Restaurant Inti"), nearby.added.map { it.name })
        assertEquals("https://www.openstreetmap.org/node/43", nearby.added.first().osmUrl)
        assertEquals(1, nearby.matched)
    }

    @Test
    fun decodesRefreshResults() {
        val gone = json.decodeFromString<RefreshResult>("""{"status":"gone","place":null}""")
        assertEquals(RefreshResult(Refreshed.GONE, null), gone)
        val updated = json.decodeFromString<RefreshResult>(
            """{"status":"updated","place":{"id":"osm-node-1","name":"Lalibela","lat":50.88,"lng":4.7,"distance_m":0}}""",
        )
        assertEquals("Lalibela", updated.place?.name)
        assertEquals(Refreshed.UNKNOWN, json.decodeFromString<RefreshResult>("""{"status":"later","place":null}""").status)
        val nearby = json.decodeFromString<NearbyResult>("""{"added":[],"matched":1}""")
        assertEquals(NearbyResult(emptyList(), 1), nearby)
    }

    @Test
    fun rewardsReadAsSentences() {
        val catalog = json.decodeFromString<Catalog>(fixture("countries"))
        assertEquals(
            "+380 XP once it is on OpenStreetMap: Oceania food would be new in Leuven.",
            Reward(380, Bonus.CONTINENT, "Leuven").pendingText("NZ", catalog),
        )
        assertEquals("+30 XP once it is on OpenStreetMap.", Reward(30).pendingText("NZ", catalog))
        val confirmed = Addition("user-1", "osm-node-1", "Lalibela", "ET", "Leuven", Bonus.COUNTRY, 130, AdditionStatus.CONFIRMED, 0, 1)
        assertTrue(confirmed.news)
        assertEquals("🇪🇹 Lalibela is on the map. +130 XP, and Ethiopia is new in Leuven!", confirmed.announcement(catalog))
        assertEquals(
            "🇪🇹 Lalibela is on the map. Someone added it before you, so it earned no XP.",
            confirmed.copy(status = AdditionStatus.DUPLICATE, xp = 0).announcement(catalog),
        )
    }

    @Test
    fun toleratesWhatANewerServerSends() {
        val discover = json.decodeFromString<Discover>(fixture("discover-newer"))
        assertEquals(BadgeKind.UNKNOWN, discover.badges[0].kind)
        assertEquals(EventStatus.UNKNOWN, discover.events[0].status)
        assertEquals(Status.DISCOVERED, discover.countries[0].status)
        val unlocked = json.decodeFromString<Unlocked>(
            """{"kind":"quest","country":null,"badge":"later","level":null,"title":"Quest done","text":"+50 XP."}""",
        )
        assertEquals(UnlockedKind.UNKNOWN, unlocked.kind)
        assertNull(unlocked.badge)
        assertEquals("Quest done. +50 XP.", unlocked.announcement(null))
    }

    @Test
    fun decodesCountryStatus() {
        val discover = json.decodeFromString<Discover>(fixture("discover"))
        assertTrue(discover.countries.any { it.status == Status.AVAILABLE })
        assertTrue(discover.countries.any { it.status == Status.DISCOVERED })
    }

    @Test
    fun decodesMissingAndUnknownAreas() {
        assertEquals(Located(), json.decodeFromString<Located>("""{"city":null,"postal_code":null}"""))
        // A kind this app doesn't know yet doesn't break decoding.
        val newer = json.decodeFromString<Area>("""{"id":"x","name":"Mitte","kind":"district","lat":52.5,"lng":13.4}""")
        assertEquals(AreaKind.CITY, newer.kind)
    }

    @Test
    fun areaProgressIsNullWhenNothingLoads() {
        assertEquals(AreaProgress(42, "Looking for restaurants"), json.decodeFromString<AreaProgress?>("""{"percent":42,"text":"Looking for restaurants"}"""))
        assertNull(json.decodeFromString<AreaProgress?>("null"))
    }

    @Test
    fun recentAreasPutTheNewestFirstWithoutDuplicates() {
        fun area(id: String) = Area(id = id, name = id, lat = 0.0, lng = 0.0)
        var recent = emptyList<Area>()
        for (id in listOf("a", "b", "c", "d", "e", "f", "g")) recent = withRecent(recent, area(id))
        assertEquals(listOf("g", "f", "e", "d", "c", "b"), recent.map { it.id })
        assertEquals(listOf("d", "g", "f", "e", "c", "b"), withRecent(recent, area("d")).map { it.id })
    }
}
