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
}

@Serializable
data class Badge(
    val id: String,
    val kind: BadgeKind,
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
data class Discover(
    val countries: List<CountryProgress>,
    val continents: List<GroupProgress>,
    val world: GroupProgress,
    val places: Int,
    @SerialName("places_visited") val placesVisited: Int,
    val badges: List<Badge>,
    val profile: Profile,
)

@Serializable
data class Place(
    val id: String,
    val name: String,
    val lat: Double,
    val lng: Double,
    val address: String? = null,
    val url: String? = null,
    val countries: List<String> = emptyList(),
    @SerialName("distance_m") val distanceM: Int,
    @SerialName("visited_at") val visitedAt: Long? = null,
    val verified: Boolean = false,
) {
    val visited: Boolean get() = visitedAt != null
}

@Serializable
data class GeoResult(val name: String, val lat: Double, val lng: Double)

@Serializable
data class VisitResult(val verified: Boolean, val profile: Profile)

@Serializable
internal data class Session(val token: String)

@Serializable
internal data class ReverseResult(val name: String? = null)

@Serializable
internal data class ErrorBody(val error: String? = null)

@Serializable
internal data class CheckInBody(val lat: Double? = null, val lng: Double? = null)
