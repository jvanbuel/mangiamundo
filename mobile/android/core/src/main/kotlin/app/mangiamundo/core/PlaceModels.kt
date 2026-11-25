package app.mangiamundo.core

import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

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

/** From `POST /api/places/{id}/visits`. */
@Serializable
data class VisitRemoved(
    /** The visits left at the place, newest first. */
    val visits: List<Visit> = emptyList(),
    val profile: Profile,
    /** The progress in the area after the removal; null when no area was sent. */
    val progress: Discover? = null,
)

@Serializable
data class ClosedResult(
    /** The progress in the area without the place; null when no area was sent. */
    val progress: Discover? = null,
)

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

/** A place to try from `/api/suggestion`, and the country the globe lands on. */
@Serializable
data class Suggestion(val country: String, val place: Place)

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
