package app.mangiamundo.core

import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
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
    val quests: MonthlyQuests? = null,
)

@Serializable
data class Place(
    val id: String,
    val name: String,
    val lat: Double,
    val lng: Double,
    val address: String? = null,
    val url: String? = null,
    val cuisine: String? = null,
    /** Null when the hours are unknown. */
    val hours: Hours? = null,
    val countries: List<String> = emptyList(),
    @SerialName("distance_m") val distanceM: Int,
    /** Newest first. Lists of places carry only the latest visit; [ApiClient.place] all of them. */
    val visits: List<Visit> = emptyList(),
    /** The place's page on openstreetmap.org. Null for places that don't come from OSM. */
    @SerialName("osm_url") val osmUrl: String? = null,
) {
    /** Null when the hours are unknown or can't be read. */
    val openNow: Boolean? get() = hours?.status?.open
    /** Short status line, e.g. "Open until 22:00". */
    val hoursText: String? get() = hours?.status?.text
    val visited: Boolean get() = visits.isNotEmpty()
    /** The latest visit was checked in on the spot. */
    val verified: Boolean get() = visits.firstOrNull()?.verified == true

    /** Whether the latest visit was on `today` in `zone`. Checking in again is for an earlier one. */
    fun visitedToday(today: LocalDate = LocalDate.now(), zone: ZoneId = ZoneId.systemDefault()): Boolean =
        visits.firstOrNull()?.let { Instant.ofEpochSecond(it.at).atZone(zone).toLocalDate() == today } == true
}

@Serializable
data class Hours(
    /** OSM opening_hours syntax. */
    val raw: String,
    /** Null when [raw] can't be parsed. */
    val status: HoursStatus? = null,
    /** This week, Monday to Sunday. Only from [ApiClient.place]; null in lists. */
    val week: List<Day>? = null,
)

/** One day of a place's week. */
@Serializable
data class Day(
    /** "Monday". */
    val name: String,
    val today: Boolean,
    /** E.g. "11:30-14:00"; empty when closed all day. */
    val open: List<String>,
)

@Serializable
data class HoursStatus(val open: Boolean, val text: String)

/** One check-in. */
@Serializable
data class Visit(
    val id: Long,
    /** Unix seconds. */
    val at: Long,
    /** Checked in within walking distance of the place. */
    val verified: Boolean = false,
)

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

/** From `POST /api/places/{id}/visits`. */
@Serializable
data class VisitResult(
    val visit: Visit,
    val profile: Profile,
    /** The progress in the area after the check-in; null when no area was sent. */
    val progress: Discover? = null,
    /** What the check-in unlocked, in the order to announce it. */
    val unlocked: List<Unlocked> = emptyList(),
)

@Serializable
enum class UnlockedKind {
    @SerialName("country") COUNTRY,
    @SerialName("badge") BADGE,
    @SerialName("event") EVENT,
    @SerialName("level") LEVEL,

    /** A kind from a newer server. Announced with its words alone. */
    UNKNOWN,
}

/** One thing a check-in unlocked, with the server's words to announce it. */
@Serializable
data class Unlocked(
    /** Unknown kinds from a newer server read as [UnlockedKind.UNKNOWN]. */
    val kind: UnlockedKind = UnlockedKind.UNKNOWN,
    /** For a country: its code, to show its flag. */
    val country: String? = null,
    /** For a badge: which medal it is. Null too for a kind this app doesn't know. */
    val badge: BadgeKind? = null,
    /** For a level: the level reached. */
    val level: Int? = null,
    val title: String,
    val text: String,
) {
    /** "🇦🇿 Azerbaijan: gold. 2 more for diamond." */
    fun announcement(catalog: Catalog?): String {
        val flag = catalog?.countries?.firstOrNull { it.code == country }?.flag?.let { "$it " }.orEmpty()
        return if (text.isEmpty()) "$flag$title" else "$flag$title. $text"
    }
}

@Serializable
internal data class Session(val token: String)

@Serializable
internal data class NewSessionBody(val device: String? = null)

@Serializable
internal data class RecoveryCode(val code: String)

/** A code that signs another device in to the account, once, until `expiresAt` (Unix seconds). */
@Serializable
data class LinkCode(val code: String, @SerialName("expires_at") val expiresAt: Long)

/** A device signed in to the account. */
@Serializable
data class Device(
    val id: String,
    /** What the app called itself; null for devices from before devices had names. */
    val name: String? = null,
    @SerialName("created_at") val createdAt: Long,
    @SerialName("last_used_at") val lastUsedAt: Long,
    /** The device this app runs on. */
    val current: Boolean = false,
)

@Serializable
data class Account(
    @SerialName("created_at") val createdAt: Long,
    /** When the current recovery code was made; null when there is none yet. */
    @SerialName("recovery_code_created_at") val recoveryCodeCreatedAt: Long? = null,
    /** Most recently used first. */
    val devices: List<Device> = emptyList(),
    /** The public page of the user's progress, like `/p/abcd2345ef`; null when it's off. */
    @SerialName("public_profile") val publicProfile: String? = null,
    /** The ways to sign in, oldest first; empty for an account that lives on its devices. */
    val identities: List<Identity> = emptyList(),
)

/** A way to sign in: Sign in with Apple, Google or an email address. */
@Serializable
data class Identity(
    /** "apple", "google" or "email". */
    val provider: String,
    val email: String? = null,
)

/** A public link to share, with its card image and a sentence to go with it. */
@Serializable
data class ShareLink(
    val url: String,
    @SerialName("image_url") val imageUrl: String,
    val text: String,
)

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

/** A place to try from `/api/suggestion`, and the country the globe lands on. */
@Serializable
data class Suggestion(val country: String, val place: Place)

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

/** What a restaurant the user adds brings to their area, decided when they add it. */
@Serializable
enum class Bonus {
    @SerialName("none") NONE,
    /** Its country had no places in the area yet. */
    @SerialName("country") COUNTRY,
    /** Its continent had no places in the area yet. */
    @SerialName("continent") CONTINENT,
}

/** XP for an added restaurant, granted once OpenStreetMap has it. */
@Serializable
data class Reward(
    val xp: Int,
    val bonus: Bonus = Bonus.NONE,
    @SerialName("area_name") val areaName: String? = null,
)

@Serializable
internal data class SeenBody(val ids: List<String>)

/** `POST /api/places` sends the place with its reward next to its fields. */
@Serializable
internal data class RewardOnly(val reward: Reward)

data class AddedPlace(val place: Place, val reward: Reward)

/** A named restaurant OpenStreetMap gives no cuisine, so it isn't on the map yet. */
@Serializable
data class UnknownPlace(
    val id: String,
    val name: String,
    val lat: Double,
    val lng: Double,
    val address: String? = null,
    @SerialName("distance_m") val distanceM: Int,
)

@Serializable
enum class AdditionStatus {
    @SerialName("pending") PENDING,
    @SerialName("confirmed") CONFIRMED,
    /** On OpenStreetMap, but someone else added it first. */
    @SerialName("duplicate") DUPLICATE,
    UNKNOWN,
}

/** A restaurant the user added, from `/api/additions`. */
@Serializable
data class Addition(
    /** The addition's own id, which never changes; `placeId` becomes the OSM place's. */
    val id: String,
    @SerialName("place_id") val placeId: String,
    val name: String,
    val country: String,
    @SerialName("area_name") val areaName: String? = null,
    val bonus: Bonus = Bonus.NONE,
    val xp: Int,
    val status: AdditionStatus = AdditionStatus.UNKNOWN,
    @SerialName("submitted_at") val submittedAt: Long,
    @SerialName("confirmed_at") val confirmedAt: Long? = null,
    val seen: Boolean = false,
) {
    /** Confirmed since the user was last told. */
    val news: Boolean
        get() = !seen && (status == AdditionStatus.CONFIRMED || status == AdditionStatus.DUPLICATE)

    /** "Lalibela is on the map. +130 XP, and Ethiopia is new in Leuven!" */
    fun announcement(catalog: Catalog?): String {
        val flag = catalog?.countries?.firstOrNull { it.code == country }?.flag?.let { "$it " }.orEmpty()
        if (status == AdditionStatus.DUPLICATE) {
            return "$flag$name is on the map. Someone added it before you, so it earned no XP."
        }
        val extra = bonusText(bonus, country, areaName, "is", catalog)
        return if (extra != null) "$flag$name is on the map. +$xp XP, and $extra!" else "$flag$name is on the map. +$xp XP."
    }
}

/** "+130 XP once it is on OpenStreetMap: Ethiopia would be new in Leuven." */
fun Reward.pendingText(country: String, catalog: Catalog?): String {
    val extra = bonusText(bonus, country, areaName, "would be", catalog)
    return "+$xp XP once it is on OpenStreetMap" + (extra?.let { ": $it." } ?: ".")
}

/** What an addition brings to its area, as "Ethiopia is new in Leuven"; null without a bonus. */
internal fun bonusText(bonus: Bonus, country: String, areaName: String?, tense: String, catalog: Catalog?): String? {
    val info = catalog?.countries?.firstOrNull { it.code == country } ?: return null
    val where = areaName?.let { " in $it" } ?: " here"
    return when (bonus) {
        Bonus.NONE -> null
        Bonus.COUNTRY -> "${info.name} $tense new$where"
        Bonus.CONTINENT -> {
            val continent = catalog.continents.firstOrNull { it.code == info.continent }?.name ?: return "${info.name} $tense new$where"
            "$continent food $tense new$where"
        }
    }
}

/** What reading a place again from OpenStreetMap changed. */
@Serializable
enum class Refreshed {
    @SerialName("updated") UPDATED,
    /** OpenStreetMap has the same as we do; a very recent edit may not have reached it yet. */
    @SerialName("unchanged") UNCHANGED,
    /** No longer on OpenStreetMap as a restaurant: hidden now, visits kept. */
    @SerialName("gone") GONE,
    UNKNOWN,
}

/** From `POST /api/places/{id}/refresh`. */
@Serializable
data class RefreshResult(val status: Refreshed = Refreshed.UNKNOWN, val place: Place? = null)

/** From `POST /api/places/nearby/refresh`. */
@Serializable
data class NearbyResult(
    /** Places that weren't listed before, nearest first. */
    val added: List<Place> = emptyList(),
    val matched: Int = 0,
)
