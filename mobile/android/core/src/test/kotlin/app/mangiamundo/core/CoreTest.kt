package app.mangiamundo.core

import java.io.File
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
    fun decodesApiResponses() {
        val json = kotlinx.serialization.json.Json { ignoreUnknownKeys = true }
        val place = json.decodeFromString<Place>(
            """{"id":"osm-node-1","name":"Lalibela","lat":50.8,"lng":4.7,"address":null,"url":null,
               "cuisine":"ethiopian","countries":["ET"],"distance_m":420,"visited_at":null,"verified":false}""",
        )
        assertFalse(place.visited)
        assertEquals(420, place.distanceM)
        assertEquals(Status.COMPLETED, CountryProgress("ET", "AF", 2, 2).status)
        assertEquals(Status.DISCOVERED, CountryProgress("ET", "AF", 2, 1).status)
    }

    @Test
    fun decodesRepeatVisitsAndToleratesNewBadgeKinds() {
        val json = kotlinx.serialization.json.Json { ignoreUnknownKeys = true }
        val place = json.decodeFromString<Place>(
            """{"id":"p","name":"Kaito","lat":50.8,"lng":4.7,"countries":["JP"],"distance_m":420,
               "visited_at":100,"verified":true,"visits":3,"last_visited_at":5000,"last_checkin_id":42,
               "last_dish":"jp-ramen","next_check_in_at":48200}""",
        )
        assertEquals(3, place.visits)
        assertEquals(42L, place.lastCheckinId)
        assertTrue(place.recent(nowSeconds = 6000))
        assertFalse(place.recent(nowSeconds = 48200))
        // A place from an older server still counts its single visit.
        val old = json.decodeFromString<Place>("""{"id":"p","name":"K","lat":0,"lng":0,"distance_m":1,"visited_at":1}""")
        assertEquals(1, old.visits)

        val badges = json.decodeFromString<List<Badge>>(
            """[{"id":"dish-1","kind":"dishes","title":"","description":"","progress":0,"target":1,"earned":false},
                {"id":"x","kind":"something-new","title":"","description":"","progress":0,"target":1,"earned":false}]""",
        )
        assertEquals(listOf(BadgeKind.DISHES, BadgeKind.OTHER), badges.map { it.kind })

        val catalog = Catalog(emptyList(), emptyList(), listOf(Dish("jp-ramen", "JP", "Ramen"), Dish("it-risotto", "IT", "Risotto")))
        assertEquals(listOf("Ramen"), catalog.dishesOf(listOf("JP", "KR")).map { it.name })
    }
}
