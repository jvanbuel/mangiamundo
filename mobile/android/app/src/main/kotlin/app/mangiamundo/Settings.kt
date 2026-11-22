package app.mangiamundo

import android.content.Context
import androidx.core.content.edit
import app.mangiamundo.core.Area
import app.mangiamundo.core.ProjectionKind
import app.mangiamundo.core.TokenStore
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.builtins.MapSerializer
import kotlinx.serialization.builtins.serializer
import kotlinx.serialization.json.Json

/**
 * The area the user is exploring: a city or a postal code, found by GPS or a search.
 * `lat`/`lng` is where the map pin goes: the GPS position, or the area's centre.
 * `options` holds the city and postal code found at the GPS position, to switch between.
 */
data class SavedLocation(
    val area: Area,
    val lat: Double,
    val lng: Double,
    val fromGps: Boolean,
    val options: List<Area> = emptyList(),
)

private val json = Json { ignoreUnknownKeys = true }
private val notTodaySerializer = MapSerializer(String.serializer(), Long.serializer())

/** Small values that survive restarts. */
class Settings(context: Context) : TokenStore {
    private val prefs = context.getSharedPreferences("mangiamundo", Context.MODE_PRIVATE)

    override var token: String?
        get() = prefs.getString("token", null)
        set(value) = prefs.edit { putString("token", value) }

    /** The user asked for celebration reminders. */
    var remindersOn: Boolean
        get() = prefs.getBoolean("reminders", false)
        set(value) = prefs.edit { putBoolean("reminders", value) }

    var projection: ProjectionKind
        get() = if (prefs.getString("projection", null) == "flat") ProjectionKind.FLAT else ProjectionKind.GLOBE
        set(value) = prefs.edit { putString("projection", if (value == ProjectionKind.FLAT) "flat" else "globe") }

    var location: SavedLocation?
        get() {
            val area = prefs.getString("area", null) ?: return null
            return runCatching {
                SavedLocation(
                    area = json.decodeFromString(Area.serializer(), area),
                    lat = Double.fromBits(prefs.getLong("lat", 0)),
                    lng = Double.fromBits(prefs.getLong("lng", 0)),
                    fromGps = prefs.getBoolean("gps", false),
                    options = json.decodeFromString(ListSerializer(Area.serializer()), prefs.getString("options", "[]")!!),
                )
            }.getOrNull()
        }
        set(value) = prefs.edit {
            remove("radius")
            remove("name")
            if (value == null) {
                remove("area")
                remove("lat")
            } else {
                putString("area", json.encodeToString(Area.serializer(), value.area))
                putString("options", json.encodeToString(ListSerializer(Area.serializer()), value.options))
                putLong("lat", value.lat.toRawBits())
                putLong("lng", value.lng.toRawBits())
                putBoolean("gps", value.fromGps)
            }
        }

    /** Areas chosen recently, newest first, for the location picker. */
    var recentAreas: List<Area>
        get() = runCatching {
            json.decodeFromString(ListSerializer(Area.serializer()), prefs.getString("recentAreas", "[]")!!)
        }.getOrDefault(emptyList())
        set(value) = prefs.edit { putString("recentAreas", json.encodeToString(ListSerializer(Area.serializer()), value)) }

    /** Places put off with "Not today", by id, with when they come back (epoch ms). */
    var notToday: Map<String, Long>
        get() = runCatching {
            json.decodeFromString(notTodaySerializer, prefs.getString("notToday", "{}")!!)
                .filterValues { it > System.currentTimeMillis() }
        }.getOrDefault(emptyMap())
        set(value) = prefs.edit { putString("notToday", json.encodeToString(notTodaySerializer, value)) }

    /** A location saved before areas existed (a point and a radius): (lat, lng, from GPS). */
    val legacyLocation: Triple<Double, Double, Boolean>?
        get() = if (prefs.contains("lat") && !prefs.contains("area")) {
            Triple(Double.fromBits(prefs.getLong("lat", 0)), Double.fromBits(prefs.getLong("lng", 0)), prefs.getBoolean("gps", false))
        } else {
            null
        }
}
