package app.mangiamundo.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class Country(
    val code: String,
    val name: String,
    val continent: String,
    val flag: String,
)

@Serializable
data class Continent(val code: String, val name: String)

@Serializable
data class Catalog(val countries: List<Country>, val continents: List<Continent>)

enum class Status { AVAILABLE, DISCOVERED, COMPLETED }

@Serializable
data class CountryProgress(
    val code: String,
    val continent: String,
    val places: Int,
    val visited: Int,
) {
    val status: Status
        get() = when {
            visited == 0 -> Status.AVAILABLE
            visited >= places -> Status.COMPLETED
            else -> Status.DISCOVERED
        }
}

@Serializable
data class GroupProgress(
    val code: String,
    val name: String,
    val available: Int,
    val discovered: Int,
    val completed: Int,
)

/** What a badge rewards. Clients pick the badge's icon and colour from it. */
@Serializable
enum class BadgeKind {
    @SerialName("visits") VISITS,
    @SerialName("countries") COUNTRIES,
    @SerialName("completed") COMPLETED,
    @SerialName("continent") CONTINENT,
    @SerialName("world") WORLD,
    @SerialName("everything") EVERYTHING,

    /** A kind from a newer server. Shown with a neutral medal. */
    UNKNOWN,
}

@Serializable
data class Badge(
    val id: String,
    /** Unknown kinds from a newer server read as [BadgeKind.UNKNOWN]. */
    val kind: BadgeKind = BadgeKind.UNKNOWN,
    val title: String,
    val description: String,
    val progress: Int,
    val target: Int,
    val earned: Boolean,
)

@Serializable
data class Profile(
    val xp: Int,
    val level: Int,
    @SerialName("level_xp") val levelXp: Int,
    @SerialName("next_level_xp") val nextLevelXp: Int,
    val visits: Int,
    val countries: Int,
)

@Serializable
enum class EventStatus {
    @SerialName("upcoming") UPCOMING,
    @SerialName("active") ACTIVE,
    @SerialName("ended") ENDED,

    /** A status from a newer server. The event is hidden. */
    UNKNOWN,
}

/** A time-limited challenge, like "Japan month", and how far the user got. */
@Serializable
data class EventProgress(
    val id: String,
    val title: String,
    val description: String,
    val countries: List<String>,
    /** First and last day, inclusive, as YYYY-MM-DD. */
    val starts: String,
    val ends: String,
    val target: Int,
    val progress: Int,
    val earned: Boolean,
    /** Unknown statuses from a newer server read as [EventStatus.UNKNOWN]. */
    val status: EventStatus = EventStatus.UNKNOWN,
    @SerialName("places_nearby") val placesNearby: Int,
)

@Serializable
data class NearbyCountry(val code: String, val places: Int)

/** Notification text, written by the server so every app says the same thing. */
@Serializable
data class Reminder(val title: String, val body: String)

@Serializable
data class Celebration(
    val id: String,
    val title: String,
    val description: String,
    val countries: List<String>,
    val starts: String,
    val ends: String,
    val approximate: Boolean = false,
    /** The celebration's countries that have places in the area. */
    val nearby: List<NearbyCountry>,
    val reminder: Reminder,
)

@Serializable
data class Discover(
    val countries: List<CountryProgress>,
    val continents: List<GroupProgress>,
    val world: GroupProgress,
    val places: Int,
    @SerialName("places_visited") val placesVisited: Int,
    val badges: List<Badge>,
    val profile: Profile,
    val events: List<EventProgress> = emptyList(),
)

@Serializable
data class Place(
    val id: String,
    val name: String,
    val lat: Double,
    val lng: Double,
    val address: String? = null,
    val url: String? = null,
    /** Raw OSM opening_hours. */
    @SerialName("opening_hours") val openingHours: String? = null,
    /** Null when the hours are unknown. */
    @SerialName("open_now") val openNow: Boolean? = null,
    /** Short status line, e.g. "Open until 22:00". */
    @SerialName("hours_text") val hoursText: String? = null,
    val countries: List<String> = emptyList(),
    @SerialName("distance_m") val distanceM: Int,
    @SerialName("visited_at") val visitedAt: Long? = null,
    val verified: Boolean = false,
) {
    val visited: Boolean get() = visitedAt != null
}

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
data class VisitResult(val verified: Boolean, val profile: Profile)

@Serializable
internal data class Session(val token: String)

@Serializable
internal data class ErrorBody(val error: String? = null)

@Serializable
internal data class CheckInBody(val lat: Double? = null, val lng: Double? = null)
