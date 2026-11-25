package app.mangiamundo.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/** How far the user got with a country's food: gold, platinum, then diamond. */
@Serializable
enum class Status {
    @SerialName("available") AVAILABLE,
    /** Gold. */
    @SerialName("discovered") DISCOVERED,
    @SerialName("platinum") PLATINUM,
    /** Diamond. */
    @SerialName("completed") COMPLETED,
}

@Serializable
data class CountryProgress(
    val code: String,
    val continent: String,
    val places: Int,
    val visited: Int,
    /** Places to visit for diamond. */
    val goal: Int,
    /** A status from a newer server reads as gold. */
    val status: Status = Status.DISCOVERED,
)

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
    @SerialName("pioneer") PIONEER,
    @SerialName("streak") STREAK,

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
    /** Restaurants the user added that OpenStreetMap now has. */
    val added: Int = 0,
    /** Months in a row with a check-in; a month without one yet doesn't break it until it's over. */
    val streak: Int = 0,
    @SerialName("best_streak") val bestStreak: Int = 0,
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

/** One of this month's quests. */
@Serializable
data class Quest(
    val id: String,
    val title: String,
    val description: String,
    val progress: Int,
    val target: Int,
    val done: Boolean,
    val xp: Int,
)

/** This month's quests, first to last day (YYYY-MM-DD, inclusive). */
@Serializable
data class MonthlyQuests(val month: String, val starts: String, val ends: String, val quests: List<Quest> = emptyList())

@Serializable
data class NearbyCountry(val code: String, val places: Int)

/**
 * A reminder the server plans, for the app to show as a local notification from `hour`
 * o'clock on `localDate` (YYYY-MM-DD) in the device's time zone.
 */
@Serializable
data class Reminder(
    /** Stable for this reminder: scheduling it again under the same key replaces it. */
    val key: String,
    @SerialName("local_date") val localDate: String,
    val hour: Int,
    val title: String,
    val body: String,
    /** Where a tap leads, relative to the web app: `/?country=JP` or `/?tab=badges`. */
    val url: String,
)

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
    val quests: MonthlyQuests? = null,
)
