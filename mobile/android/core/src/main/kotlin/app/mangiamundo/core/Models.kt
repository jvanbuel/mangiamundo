package app.mangiamundo.core

import kotlinx.serialization.KSerializer
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.descriptors.PrimitiveKind
import kotlinx.serialization.descriptors.PrimitiveSerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder

@Serializable
data class Country(
    val code: String,
    val name: String,
    val continent: String,
    val flag: String,
)

@Serializable
data class Continent(val code: String, val name: String)

/** A signature dish of a country; logging one at a check-in earns bonus XP. */
@Serializable
data class Dish(val id: String, val country: String, val name: String)

@Serializable
data class Catalog(
    val countries: List<Country>,
    val continents: List<Continent>,
    val dishes: List<Dish> = emptyList(),
) {
    /** Signature dishes of the given countries, in catalog order. */
    fun dishesOf(codes: Collection<String>): List<Dish> = dishes.filter { it.country in codes }
}

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

/**
 * What a badge rewards. Clients pick the badge's icon and colour from it. Kinds this
 * version does not know yet decode as [OTHER], so a newer server cannot break the app.
 */
@Serializable(with = BadgeKindSerializer::class)
enum class BadgeKind(val wire: String) {
    VISITS("visits"),
    COUNTRIES("countries"),
    COMPLETED("completed"),
    CONTINENT("continent"),
    WORLD("world"),
    EVERYTHING("everything"),
    DISHES("dishes"),
    REGULAR("regular"),
    OTHER(""),
}

object BadgeKindSerializer : KSerializer<BadgeKind> {
    override val descriptor = PrimitiveSerialDescriptor("BadgeKind", PrimitiveKind.STRING)
    override fun serialize(encoder: Encoder, value: BadgeKind) = encoder.encodeString(value.wire)
    override fun deserialize(decoder: Decoder): BadgeKind {
        val wire = decoder.decodeString()
        return BadgeKind.entries.firstOrNull { it.wire == wire } ?: BadgeKind.OTHER
    }
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
    val dishes: Int = 0,
)

@Serializable
enum class EventStatus {
    @SerialName("upcoming") UPCOMING,
    @SerialName("active") ACTIVE,
    @SerialName("ended") ENDED,
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
    val status: EventStatus,
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
    /** Ids of every signature dish the user logged. */
    @SerialName("dishes_tried") val dishesTried: List<String> = emptyList(),
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
    /** First check-in; set means the place counts as visited. */
    @SerialName("visited_at") val visitedAt: Long? = null,
    val verified: Boolean = false,
    /** Number of check-ins here. */
    val visits: Int = if (visitedAt != null) 1 else 0,
    @SerialName("last_visited_at") val lastVisitedAt: Long? = null,
    @SerialName("last_checkin_id") val lastCheckinId: Long? = null,
    @SerialName("last_dish") val lastDish: String? = null,
    /** When checking in again becomes possible, if not right now (Unix seconds). */
    @SerialName("next_check_in_at") val nextCheckInAt: Long? = null,
) {
    val visited: Boolean get() = visitedAt != null

    /** Checked in within the cooldown: the time to log a dish or take the check-in back. */
    fun recent(nowSeconds: Long): Boolean = nextCheckInAt != null && nextCheckInAt > nowSeconds
}

/** Server rules the app mirrors for optimistic updates and messages. */
object Rules {
    const val CHECKIN_COOLDOWN_SECS = 12 * 3600L
    const val XP_PER_VISIT = 10
    const val XP_PER_DISH = 25
}

@Serializable
data class GeoResult(val name: String, val lat: Double, val lng: Double)

@Serializable
data class VisitResult(
    val verified: Boolean,
    val profile: Profile,
    @SerialName("checkin_id") val checkinId: Long = 0,
)

@Serializable
data class RecoveryCode(val code: String)

@Serializable
internal data class Restored(val token: String, val merged: Int)

@Serializable
internal data class RestoreBody(val code: String)

@Serializable
internal data class DishBody(val dish: String?)

@Serializable
internal data class Session(val token: String)

@Serializable
internal data class ReverseResult(val name: String? = null)

@Serializable
internal data class ErrorBody(val error: String? = null)

@Serializable
internal data class CheckInBody(val lat: Double? = null, val lng: Double? = null)
