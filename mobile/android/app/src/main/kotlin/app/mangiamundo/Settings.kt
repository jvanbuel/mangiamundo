package app.mangiamundo

import android.content.Context
import androidx.core.content.edit
import app.mangiamundo.core.ProjectionKind
import app.mangiamundo.core.TokenStore

/** The place the user is exploring: their GPS position or a searched location. */
data class SavedLocation(val lat: Double, val lng: Double, val name: String?, val fromGps: Boolean)

/** Small values that survive restarts. */
class Settings(context: Context) : TokenStore {
    private val prefs = context.getSharedPreferences("mangiamundo", Context.MODE_PRIVATE)

    override var token: String?
        get() = prefs.getString("token", null)
        set(value) = prefs.edit { putString("token", value) }

    var radiusKm: Int
        get() = prefs.getInt("radius", 10)
        set(value) = prefs.edit { putInt("radius", value) }

    var projection: ProjectionKind
        get() = if (prefs.getString("projection", null) == "flat") ProjectionKind.FLAT else ProjectionKind.GLOBE
        set(value) = prefs.edit { putString("projection", if (value == ProjectionKind.FLAT) "flat" else "globe") }

    var location: SavedLocation?
        get() {
            if (!prefs.contains("lat")) return null
            return SavedLocation(
                lat = Double.fromBits(prefs.getLong("lat", 0)),
                lng = Double.fromBits(prefs.getLong("lng", 0)),
                name = prefs.getString("name", null),
                fromGps = prefs.getBoolean("gps", false),
            )
        }
        set(value) = prefs.edit {
            if (value == null) {
                remove("lat")
            } else {
                putLong("lat", value.lat.toRawBits())
                putLong("lng", value.lng.toRawBits())
                putString("name", value.name)
                putBoolean("gps", value.fromGps)
            }
        }
}
