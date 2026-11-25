package app.mangiamundo.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

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

/** `POST /api/places` sends the place with its reward next to its fields. */
@Serializable
internal data class RewardOnly(val reward: Reward)

data class AddedPlace(val place: Place, val reward: Reward)

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
