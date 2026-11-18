package app.mangiamundo.core

import java.io.File
import java.time.LocalDate
import java.time.ZoneId
import kotlin.math.abs
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class CoreTest {
    private val world = World.parse(File("../../shared/world.json").readText())

    @Test
    fun findsCountriesByPosition() {
        assertEquals("BE", world.countryAt(4.70, 50.88)?.code)
        assertEquals("JP", world.countryAt(139.65, 35.68)?.code)
        assertEquals("BR", world.countryAt(-47.9, -15.8)?.code)
        // Eastern Russia is stored past the antimeridian (lng > 180).
        assertEquals("RU", world.countryAt(-172.0, 65.5)?.code)
        assertEquals("RU", world.countryAt(100.0, 62.0)?.code)
        // Lesotho is a hole in South Africa.
        assertEquals("LS", world.countryAt(28.2, -29.6)?.code)
        assertNull(world.countryAt(-30.0, 30.0))
    }

    @Test
    fun projectionsRoundTrip() {
        val out = FloatArray(2)
        for (kind in ProjectionKind.entries) {
            val projector = Projector(kind, Camera(4.7, 50.9, 1.5), 400.0, 600.0)
            for ((lng, lat) in listOf(4.7 to 50.9, -3.7 to 40.4, 30.5 to 50.4, 12.5 to 41.9)) {
                assertTrue(projector.project(lng, lat, out))
                val back = assertNotNull(projector.unproject(out[0].toDouble(), out[1].toDouble()))
                assertTrue(abs(back.first - lng) < 0.01 && abs(back.second - lat) < 0.01, "$kind $lng,$lat -> $back")
            }
            // The camera centre lands in the middle of the view.
            projector.project(4.7, 50.9, out)
            assertEquals(200f, out[0], 0.01f)
            assertEquals(300f, out[1], 0.01f)
        }
    }

    @Test
    fun globeHidesTheFarSide() {
        val projector = Projector(ProjectionKind.GLOBE, Camera(0.0, 0.0, 1.0), 400.0, 400.0)
        val out = FloatArray(2)
        assertFalse(projector.project(170.0, 10.0, out))
        // Hidden points are pushed to the rim of the disc.
        val fromCentre = kotlin.math.hypot(out[0] - 200.0, out[1] - 200.0)
        assertEquals(projector.radius, fromCentre, 0.01)
        assertNull(projector.unproject(2.0, 2.0))
    }

    @Test
    fun cameraStaysInBounds() {
        val camera = Camera(190.0, 95.0, 50.0).normalized()
        assertEquals(-170.0, camera.lng, 1e-9)
        assertEquals(Camera.MAX_LAT, camera.lat)
        assertEquals(Camera.MAX_ZOOM, camera.zoom)
    }

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
    private val json = ApiClient("http://localhost", object : TokenStore { override var token: String? = null }).json
    private fun fixture(name: String) = File("../../../contract/$name.json").readText()

    @Test
    fun decodesContractFixtures() {
        assertTrue(json.decodeFromString<Catalog>(fixture("countries")).countries.size > 200)
        val discover = json.decodeFromString<Discover>(fixture("discover"))
        assertEquals(2, discover.placesVisited)
        assertTrue(discover.events.isNotEmpty())
        val places = json.decodeFromString<List<Place>>(fixture("places"))
        assertEquals(2, places.count { it.visited })
        assertEquals(1, places.count { it.verified })
        val celebrations = json.decodeFromString<List<Celebration>>(fixture("celebrations"))
        assertTrue(celebrations.all { it.nearby.isNotEmpty() && it.reminder.title.isNotEmpty() })
        assertTrue(places.any { it.hours?.status != null })
        assertTrue(json.decodeFromString<VisitResult>(fixture("visit")).visit.verified)
        val place = json.decodeFromString<Place>(fixture("place"))
        assertEquals(1, place.visits.size)
        assertTrue(place.verified)
        assertEquals(7, place.hours?.week?.size)
        assertEquals(1, place.hours?.week?.count { it.today })
        assertTrue(places.all { it.hours?.week == null })
        assertTrue(json.decodeFromString<List<Area>>(fixture("areas")).isNotEmpty())
        assertEquals(2, json.decodeFromString<Located>(fixture("located")).options.size)
        assertEquals("area not found", json.decodeFromString<ErrorBody>(fixture("error")).error)
    }

    @Test
    fun toleratesWhatANewerServerSends() {
        val discover = json.decodeFromString<Discover>(fixture("discover-newer"))
        assertEquals(BadgeKind.UNKNOWN, discover.badges[0].kind)
        assertEquals(EventStatus.UNKNOWN, discover.events[0].status)
    }

    @Test
    fun completionStatus() {
        assertEquals(Status.COMPLETED, CountryProgress("ET", "AF", 2, 2).status)
        assertEquals(Status.DISCOVERED, CountryProgress("ET", "AF", 2, 1).status)
    }

    @Test
    fun decodesMissingAndUnknownAreas() {
        assertEquals(Located(), json.decodeFromString<Located>("""{"city":null,"postal_code":null}"""))
        // A kind this app doesn't know yet doesn't break decoding.
        val newer = json.decodeFromString<Area>("""{"id":"x","name":"Mitte","kind":"district","lat":52.5,"lng":13.4}""")
        assertEquals(AreaKind.CITY, newer.kind)
    }
}
