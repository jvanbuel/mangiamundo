package app.mangiamundo.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class Country(
    val code: String,
    val name: String,
    val continent: String,
    val flag: String,
    /** Its food's OpenStreetMap `cuisine` value; only countries with one can be picked for a new place. */
    val cuisine: String? = null,
)

/** From `/api/health`. An older server sends only `ok`. */
@Serializable
data class Health(val version: String? = null, val commit: String? = null)

@Serializable
data class Continent(val code: String, val name: String)

@Serializable
data class Catalog(val countries: List<Country>, val continents: List<Continent>)

@Serializable
data class Area(
    /** Opaque, e.g. "osm-relation-196311". */
    val id: String,
    /** "Leuven", or "3000 Leuven" for a postal code. */
    val name: String,
    /** Unknown kinds from a newer server read as a city. */
    val kind: AreaKind = AreaKind.CITY,
    /** Where it is, to tell same-named areas apart: "Flemish Brabant, Belgium". */
    val detail: String? = null,
    val lat: Double,
    val lng: Double,
)

/** Recent areas with `area` in front: newest first, no duplicates, at most 6. */
fun withRecent(recent: List<Area>, area: Area): List<Area> =
    (listOf(area) + recent.filter { it.id != area.id }).take(6)

@Serializable
enum class AreaKind {
    /** A city, town or municipality: every postal code in it. */
    @SerialName("city") CITY,
    @SerialName("postal_code") POSTAL_CODE,
}

/** The areas a point lies in. */
@Serializable
data class Located(
    val city: Area? = null,
    @SerialName("postal_code") val postalCode: Area? = null,
) {
    val options: List<Area> get() = listOfNotNull(city, postalCode)
}

@Serializable
internal data class Session(val token: String)

@Serializable
internal data class NewSessionBody(val device: String? = null)

@Serializable
internal data class RecoveryCode(val code: String)

/** How far the server is with loading an area, for the progress wheel. */
@Serializable
data class AreaProgress(
    /** 0 to 100. */
    val percent: Int,
    /** What is happening, e.g. "Looking for restaurants". */
    val text: String,
)

@Serializable
internal data class ErrorBody(val error: String? = null, val code: String? = null)

@Serializable
internal data class CheckInBody(val position: LatLng? = null)

@Serializable
internal data class AddPlaceBody(
    val name: String,
    val country: String,
    val lat: Double,
    val lng: Double,
    val address: String? = null,
    val website: String? = null,
    val note: Boolean = true,
)

@Serializable
internal data class LatLng(val lat: Double, val lng: Double)

@Serializable
internal data class SeenBody(val ids: List<String>)
