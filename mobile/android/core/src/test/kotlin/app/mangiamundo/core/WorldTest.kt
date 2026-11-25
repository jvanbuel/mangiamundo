package app.mangiamundo.core

import java.io.File
import kotlin.math.abs
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class WorldTest {
    private val world = World.parse(File("../../../shared/world.json").readText())

    @Test
    fun spinsLandOnTheMainland() {
        val france = world.countries.first { it.code == "FR" }.landing()
        assertTrue(abs(france.lng - 2.5) < 4 && abs(france.lat - 46.5) < 4, "$france")
        val belgium = world.countries.first { it.code == "BE" }.landing()
        assertEquals("BE", world.countryAt(belgium.lng, belgium.lat)?.code)
        assertEquals(4.0, belgium.zoom)
    }

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
    fun groupsBorderingCountries() {
        val regions = world.regions(setOf("BE", "NL", "FR", "JP"))
        assertEquals(regions["BE"], regions["NL"])
        assertEquals(regions["BE"], regions["FR"])
        assertTrue(regions["JP"] != regions["BE"])
        assertFalse("PL" in world.neighbours.getValue("FR"))
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
}
