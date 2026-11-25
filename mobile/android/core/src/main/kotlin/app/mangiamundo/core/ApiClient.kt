package app.mangiamundo.core

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import java.net.HttpURLConnection
import java.net.URL
import java.net.URLEncoder

/** `code` is the server's error code, such as `nothing_open`. */
class ApiException(val status: Int, message: String, val code: String? = null) : Exception(message)

/** Where the anonymous account token of this device is kept. */
interface TokenStore {
    var token: String?
}

/**
 * `deviceName` is how this device shows up in the account's device list, such as "Pixel 8".
 *
 * The device gets its account with the first thing it changes (a check-in, an added place):
 * what it only reads, it reads without one. `onSignedOut` is called when the server turns out
 * not to know the device's token any more, after the token is dropped: the device was signed
 * out from another one, or its account was deleted. No new account is made then; that waits
 * for the next change.
 */
class ApiClient(
    private val baseUrl: String,
    private val tokens: TokenStore,
    private val deviceName: String? = null,
    private val onSignedOut: () -> Unit = {},
) {
    @PublishedApi
    internal val json = Json {
        ignoreUnknownKeys = true
        // Unknown enum values fall back to the property's default where it has one.
        coerceInputValues = true
    }
    private val http = ApiTransport(baseUrl, tokens, deviceName, onSignedOut, json)

    suspend fun health(): Health = json.decodeFromString(http.request("GET", "/api/health"))

    suspend fun catalog(): Catalog = json.decodeFromString(http.request("GET", "/api/countries"))

    suspend fun discover(areaId: String): Discover =
        json.decodeFromString(http.read("/api/discover?${area(areaId)}&${tz()}"))

    /** Distances are measured from `from` (lat, lng) when given, otherwise from the area's centre. */
    suspend fun places(areaId: String, country: String, from: Pair<Double, Double>?): List<Place> {
        val position = from?.let { "&lat=${it.first}&lng=${it.second}" }.orEmpty()
        return json.decodeFromString(http.read("/api/places?${area(areaId)}&country=$country$position"))
    }

    /** A random unvisited place to try, open now unless `open` is false, skipping `exclude`. */
    suspend fun suggestion(areaId: String, from: Pair<Double, Double>?, exclude: Collection<String>, open: Boolean): Suggestion {
        val position = from?.let { "&lat=${it.first}&lng=${it.second}" }.orEmpty()
        val skip = exclude.joinToString(",") { encode(it) }
        return json.decodeFromString(http.read("/api/suggestion?${area(areaId)}&open=$open&exclude=$skip$position"))
    }

    suspend fun celebrations(areaId: String, days: Int = 60): List<Celebration> =
        json.decodeFromString(http.request("GET", "/api/celebrations?${area(areaId)}&days=$days&${tz()}"))

    /** The reminders that become due in the area in the coming `days`, soonest first, for this user. */
    suspend fun reminders(areaId: String, days: Int = 60): List<Reminder> =
        json.decodeFromString(http.read("/api/reminders?${area(areaId)}&days=$days&${tz()}"))

    /** One place with all of the user's visits. The distance is measured from `from` (lat, lng) when given. */
    suspend fun place(placeId: String, from: Pair<Double, Double>?): Place {
        val position = from?.let { "?lat=${it.first}&lng=${it.second}" }.orEmpty()
        return json.decodeFromString(http.read("/api/places/${encode(placeId)}$position"))
    }

    /**
     * Checks in, and answers with the progress in the area afterwards and what the check-in
     * unlocked. `here` is the device position, sent so the server can mark the visit as verified.
     */
    suspend fun checkIn(placeId: String, areaId: String, here: Pair<Double, Double>?): VisitResult {
        val position = here?.let { LatLng(it.first, it.second) }
        val body = json.encodeToString(CheckInBody.serializer(), CheckInBody(position))
        return json.decodeFromString(http.write("POST", "/api/places/${encode(placeId)}/visits?${area(areaId)}&${tz()}", body))
    }

    /** Takes a check-in back. The answer has the visits left at the place and the progress in the area. */
    suspend fun removeVisit(visitId: Long, areaId: String): VisitRemoved =
        json.decodeFromString(http.write("DELETE", "/api/visits/$visitId?${area(areaId)}&${tz()}"))

    /** Makes (or moves to this area) the public page of the user's progress. */
    suspend fun shareProgress(areaId: String): ShareLink =
        json.decodeFromString(http.write("POST", "/api/share?${area(areaId)}"))

    /** Turns the public page off. */
    suspend fun unshareProgress() {
        http.writeMine("DELETE", "/api/share")
    }

    /** A public page for one check-in. */
    suspend fun shareVisit(visitId: Long): ShareLink =
        json.decodeFromString(http.write("POST", "/api/visits/$visitId/share"))

    /** The share card; null when it can't be loaded, so the link is shared alone. */
    suspend fun card(link: ShareLink): ByteArray? = withContext(Dispatchers.IO) {
        runCatching {
            val connection = URL(link.imageUrl).openConnection() as HttpURLConnection
            connection.connectTimeout = 10_000
            connection.readTimeout = 15_000
            try {
                if (connection.responseCode == 200) connection.inputStream.use { it.readBytes() } else null
            } finally {
                connection.disconnect()
            }
        }.getOrNull()
    }

    /** "This place has closed": hides it for this user, and for everyone after a second report. */
    /** With an area, the answer has the progress there without the place. */
    suspend fun reportClosed(placeId: String, areaId: String? = null): ClosedResult {
        val query = areaId?.let { "${area(it)}&" }.orEmpty()
        return json.decodeFromString(http.write("PUT", "/api/places/${encode(placeId)}/closed?$query${tz()}"))
    }

    /** How far loading an area has come while `discover` waits on it; null when it isn't loading. */
    suspend fun areaProgress(areaId: String): AreaProgress? =
        json.decodeFromString(http.request("GET", "/api/areas/${encode(areaId)}/progress"))

    /** Adds a restaurant OpenStreetMap lacks. Only this user sees it until OSM has it. */
    /** `note = false` when the user adds it to OSM themselves, so no note asks mappers to. */
    suspend fun addPlace(name: String, country: String, lat: Double, lng: Double, address: String?, website: String?, note: Boolean = true): AddedPlace {
        val body = json.encodeToString(AddPlaceBody.serializer(), AddPlaceBody(name, country, lat, lng, address, website, note))
        return decodeAdded(http.write("POST", "/api/places", body))
    }

    internal fun decodeAdded(text: String): AddedPlace =
        AddedPlace(json.decodeFromString<Place>(text), json.decodeFromString<RewardOnly>(text).reward)

    /** Restaurants in the area OpenStreetMap gives no cuisine, nearest to `from` first. */
    suspend fun unknownPlaces(areaId: String, from: Pair<Double, Double>?): List<UnknownPlace> {
        val position = from?.let { "&lat=${it.first}&lng=${it.second}" }.orEmpty()
        return json.decodeFromString(http.read("/api/unknown-places?${area(areaId)}$position"))
    }

    /** "It serves food from …": like adding a place, only this user sees it until OSM has the cuisine. */
    suspend fun suggestCuisine(placeId: String, country: String): AddedPlace =
        decodeAdded(http.write("POST", "/api/unknown-places/${encode(placeId)}/cuisine", jsonObject(mapOf("country" to country))))

    /** "It serves no particular country's food": hides it for this user, and for everyone after two. */
    suspend fun noCuisine(placeId: String) {
        http.write("PUT", "/api/unknown-places/${encode(placeId)}/no-cuisine")
    }

    /** Reads a place again from OpenStreetMap, for an edit made there since the daily import. */
    suspend fun refreshPlace(placeId: String, from: Pair<Double, Double>?): RefreshResult {
        val position = from?.let { "?lat=${it.first}&lng=${it.second}" }.orEmpty()
        return json.decodeFromString(http.write("POST", "/api/places/${encode(placeId)}/refresh$position"))
    }

    /** Asks OpenStreetMap for restaurants within 150 m of (lat, lng), for one just mapped there. */
    suspend fun refreshNearby(lat: Double, lng: Double): NearbyResult =
        json.decodeFromString(http.write("POST", "/api/places/nearby/refresh?lat=$lat&lng=$lng"))

    /** The restaurants this user added, newest first, with their rewards. */
    suspend fun additions(): List<Addition> = http.mine("/api/additions")?.let { json.decodeFromString(it) } ?: emptyList()

    /** The user was told about these confirmed additions, by `id` (at most 100). */
    suspend fun markAdditionsSeen(ids: List<String>) {
        http.writeMine("PUT", "/api/additions/seen", json.encodeToString(SeenBody.serializer(), SeenBody(ids)))
    }

    /** Cities and postal codes matching what someone typed. */
    suspend fun searchAreas(query: String): List<Area> =
        json.decodeFromString(http.request("GET", "/api/areas?q=${encode(query)}"))

    /** The city and postal code a point lies in. */
    suspend fun locateArea(lat: Double, lng: Double): Located =
        json.decodeFromString(http.request("GET", "/api/areas/locate?lat=$lat&lng=$lng"))

    /** Whether this device has an account yet; it gets one with its first check-in or other change. */
    val hasAccount: Boolean get() = tokens.token != null

    /**
     * The account's recovery code status and the devices signed in to it. A device without an
     * account has an empty one, with no devices: nothing is on the server until its first change.
     */
    suspend fun account(): Account = http.mine("/api/account")?.let { json.decodeFromString(it) } ?: Account(createdAt = 0)

    /** A new recovery code, shown once. The previous one stops working. */
    suspend fun newRecoveryCode(): String =
        json.decodeFromString<RecoveryCode>(http.write("POST", "/api/account/recovery-code")).code

    /** A code that signs another device in to this account, once, within 10 minutes. */
    suspend fun newLinkCode(): LinkCode = json.decodeFromString(http.write("POST", "/api/account/link-code"))

    /**
     * Signs this device in to the account of a recovery or link code. What this device did
     * before moves into that account. An unknown, used or expired code is a 404.
     */
    suspend fun restore(code: String) = http.switchAccount("/api/session/restore", mapOf("code" to code))

    /** Emails a 6-digit sign-in code. */
    suspend fun sendEmailCode(email: String) {
        http.request("POST", "/api/auth/email/code", jsonObject(mapOf("email" to email)))
    }

    /** Signs in with an emailed code. A wrong, used or expired code is a 404. */
    suspend fun signIn(email: String, code: String) =
        http.switchAccount("/api/auth/email", mapOf("email" to email, "code" to code))

    /** Signs in with an ID token from Google's sign-in. */
    suspend fun signInWithGoogle(idToken: String) = http.switchAccount("/api/auth/google", mapOf("id_token" to idToken))

    /** Signs this device out of its account; it starts afresh, without one. */
    suspend fun signOut() {
        account().devices.firstOrNull { it.current }?.let { removeDevice(it.id) }
        http.dropToken()
    }

    /** Signs another device out of this account. */
    suspend fun removeDevice(id: String) {
        http.writeMine("DELETE", "/api/account/devices/${encode(id)}")
    }

    /** Deletes the account and everything in it. This device starts over without one. */
    suspend fun deleteAccount() {
        http.writeMine("DELETE", "/api/account")
        http.dropToken()
    }

    private fun area(id: String) = "area=${encode(id)}"

    /** The device's time zone, so event days begin and end at local midnight. */
    private fun tz() = "tz=${encode(java.util.TimeZone.getDefault().id)}"

    private fun encode(value: String) = URLEncoder.encode(value, "UTF-8").replace("+", "%20")
}
