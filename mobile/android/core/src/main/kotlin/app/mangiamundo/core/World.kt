package app.mangiamundo.core

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

/** A country outline from `world.json`. */
class WorldCountry(
    val code: String,
    val name: String,
    /** Rings as flat `[lng, lat, lng, lat, ...]` arrays. Holes are rings too: fill even-odd. */
    val rings: List<DoubleArray>,
) {
    val minLng: Double
    val maxLng: Double
    val minLat: Double
    val maxLat: Double

    init {
        var w = Double.MAX_VALUE
        var e = -Double.MAX_VALUE
        var s = Double.MAX_VALUE
        var n = -Double.MAX_VALUE
        for (ring in rings) {
            for (i in ring.indices step 2) {
                w = minOf(w, ring[i]); e = maxOf(e, ring[i])
                s = minOf(s, ring[i + 1]); n = maxOf(n, ring[i + 1])
            }
        }
        minLng = w; maxLng = e; minLat = s; maxLat = n
    }

    /** Even-odd point-in-polygon test. */
    fun contains(lng: Double, lat: Double): Boolean {
        if (lat < minLat || lat > maxLat || lng < minLng || lng > maxLng) return false
        var inside = false
        for (ring in rings) {
            var j = ring.size - 2
            for (i in ring.indices step 2) {
                val (xi, yi) = ring[i] to ring[i + 1]
                val (xj, yj) = ring[j] to ring[j + 1]
                if ((yi > lat) != (yj > lat) && lng < (xj - xi) * (lat - yi) / (yj - yi) + xi) inside = !inside
                j = i
            }
        }
        return inside
    }
}

class World(val countries: List<WorldCountry>) {
    /** The country at a position, or null over the sea. */
    fun countryAt(lng: Double, lat: Double): WorldCountry? {
        // Some rings run past the antimeridian (lng > 180 or < -180), so try the wrapped copies.
        for (shift in doubleArrayOf(0.0, 360.0, -360.0)) {
            countries.firstOrNull { it.contains(lng + shift, lat) }?.let { return it }
        }
        return null
    }

    companion object {
        @Serializable
        private class Raw(val code: String, val name: String, val rings: List<DoubleArray>)

        fun parse(json: String): World =
            World(Json.decodeFromString<List<Raw>>(json).map { WorldCountry(it.code, it.name, it.rings) })
    }
}
