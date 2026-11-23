package app.mangiamundo.core

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL
import java.net.URLEncoder

/** `code` is the server's error code, such as `nothing_open`. */
class ApiException(val status: Int, message: String, val code: String? = null) : Exception(message)

/** Where the anonymous account token of this device is kept. */
interface TokenStore {
    var token: String?
}

/** `deviceName` is how this device shows up in the account's device list, such as "Pixel 8". */
class ApiClient(private val baseUrl: String, private val tokens: TokenStore, private val deviceName: String? = null) {
    @PublishedApi
    internal val json = Json {
        ignoreUnknownKeys = true
        // Unknown enum values fall back to the property's default where it has one.
        coerceInputValues = true
    }
    private val tokenLock = Mutex()

    suspend fun health(): Health = json.decodeFromString(request("GET", "/api/health"))

    suspend fun catalog(): Catalog = json.decodeFromString(request("GET", "/api/countries"))

    suspend fun discover(areaId: String): Discover =
        json.decodeFromString(authed("GET", "/api/discover?${area(areaId)}&${tz()}"))

    /** Distances are measured from `from` (lat, lng) when given, otherwise from the area's centre. */
    suspend fun places(areaId: String, country: String, from: Pair<Double, Double>?): List<Place> {
        val position = from?.let { "&lat=${it.first}&lng=${it.second}" }.orEmpty()
        return json.decodeFromString(authed("GET", "/api/places?${area(areaId)}&country=$country$position"))
    }

    /** A random unvisited place to try, open now unless `open` is false, skipping `exclude`. */
    suspend fun suggestion(areaId: String, from: Pair<Double, Double>?, exclude: Collection<String>, open: Boolean): Suggestion {
        val position = from?.let { "&lat=${it.first}&lng=${it.second}" }.orEmpty()
        val skip = exclude.joinToString(",") { encode(it) }
        return json.decodeFromString(authed("GET", "/api/suggestion?${area(areaId)}&open=$open&exclude=$skip$position"))
    }

    suspend fun celebrations(areaId: String, days: Int = 60): List<Celebration> =
        json.decodeFromString(request("GET", "/api/celebrations?${area(areaId)}&days=$days&${tz()}"))

    /** One place with all of the user's visits. The distance is measured from `from` (lat, lng) when given. */
    suspend fun place(placeId: String, from: Pair<Double, Double>?): Place {
        val position = from?.let { "?lat=${it.first}&lng=${it.second}" }.orEmpty()
        return json.decodeFromString(authed("GET", "/api/places/${encode(placeId)}$position"))
    }

    /** `here` is the device position, sent so the server can mark the visit as verified. */
    suspend fun checkIn(placeId: String, here: Pair<Double, Double>?): VisitResult {
        val position = here?.let { LatLng(it.first, it.second) }
        val body = json.encodeToString(CheckInBody.serializer(), CheckInBody(position))
        return json.decodeFromString(authed("POST", "/api/places/${encode(placeId)}/visits?${tz()}", body))
    }

    suspend fun removeVisit(visitId: Long) {
        authed("DELETE", "/api/visits/$visitId")
    }

    /** Makes (or moves to this area) the public page of the user's progress. */
    suspend fun shareProgress(areaId: String): ShareLink =
        json.decodeFromString(authed("POST", "/api/share?${area(areaId)}"))

    /** Turns the public page off. */
    suspend fun unshareProgress() {
        authed("DELETE", "/api/share")
    }

    /** A public page for one check-in. */
    suspend fun shareVisit(visitId: Long): ShareLink =
        json.decodeFromString(authed("POST", "/api/visits/$visitId/share"))

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
    suspend fun reportClosed(placeId: String) {
        authed("PUT", "/api/places/${encode(placeId)}/closed")
    }

    /** How far loading an area has come while `discover` waits on it; null when it isn't loading. */
    suspend fun areaProgress(areaId: String): AreaProgress? =
        json.decodeFromString(request("GET", "/api/areas/${encode(areaId)}/progress"))

    /** Adds a restaurant OpenStreetMap lacks. Only this user sees it until OSM has it. */
    /** `note = false` when the user adds it to OSM themselves, so no note asks mappers to. */
    suspend fun addPlace(name: String, country: String, lat: Double, lng: Double, address: String?, website: String?, note: Boolean = true): AddedPlace {
        val body = json.encodeToString(AddPlaceBody.serializer(), AddPlaceBody(name, country, lat, lng, address, website, note))
        return decodeAdded(authed("POST", "/api/places", body))
    }

    internal fun decodeAdded(text: String): AddedPlace =
        AddedPlace(json.decodeFromString<Place>(text), json.decodeFromString<RewardOnly>(text).reward)

    /** Restaurants in the area OpenStreetMap gives no cuisine, nearest to `from` first. */
    suspend fun unknownPlaces(areaId: String, from: Pair<Double, Double>?): List<UnknownPlace> {
        val position = from?.let { "&lat=${it.first}&lng=${it.second}" }.orEmpty()
        return json.decodeFromString(authed("GET", "/api/unknown-places?${area(areaId)}$position"))
    }

    /** "It serves food from …": like adding a place, only this user sees it until OSM has the cuisine. */
    suspend fun suggestCuisine(placeId: String, country: String): AddedPlace =
        decodeAdded(authed("POST", "/api/unknown-places/${encode(placeId)}/cuisine", jsonObject(mapOf("country" to country))))

    /** "It serves no particular country's food": hides it for this user, and for everyone after two. */
    suspend fun noCuisine(placeId: String) {
        authed("PUT", "/api/unknown-places/${encode(placeId)}/no-cuisine")
    }

    /** Reads a place again from OpenStreetMap, for an edit made there since the daily import. */
    suspend fun refreshPlace(placeId: String, from: Pair<Double, Double>?): RefreshResult {
        val position = from?.let { "?lat=${it.first}&lng=${it.second}" }.orEmpty()
        return json.decodeFromString(authed("POST", "/api/places/${encode(placeId)}/refresh$position"))
    }

    /** Asks OpenStreetMap for restaurants within 150 m of (lat, lng), for one just mapped there. */
    suspend fun refreshNearby(lat: Double, lng: Double): NearbyResult =
        json.decodeFromString(authed("POST", "/api/places/nearby/refresh?lat=$lat&lng=$lng"))

    /** The restaurants this user added, newest first, with their rewards. */
    suspend fun additions(): List<Addition> = json.decodeFromString(authed("GET", "/api/additions"))

    /** The user was told about these confirmed additions, by `id` (at most 100). */
    suspend fun markAdditionsSeen(ids: List<String>) {
        authed("PUT", "/api/additions/seen", json.encodeToString(SeenBody.serializer(), SeenBody(ids)))
    }

    /** Cities and postal codes matching what someone typed. */
    suspend fun searchAreas(query: String): List<Area> =
        json.decodeFromString(request("GET", "/api/areas?q=${encode(query)}"))

    /** The city and postal code a point lies in. */
    suspend fun locateArea(lat: Double, lng: Double): Located =
        json.decodeFromString(request("GET", "/api/areas/locate?lat=$lat&lng=$lng"))

    /** The account's recovery code status and the devices signed in to it. */
    suspend fun account(): Account = json.decodeFromString(authed("GET", "/api/account"))

    /** A new recovery code, shown once. The previous one stops working. */
    suspend fun newRecoveryCode(): String =
        json.decodeFromString<RecoveryCode>(authed("POST", "/api/account/recovery-code")).code

    /** A code that signs another device in to this account, once, within 10 minutes. */
    suspend fun newLinkCode(): LinkCode = json.decodeFromString(authed("POST", "/api/account/link-code"))

    /**
     * Signs this device in to the account of a recovery or link code. What this device did
     * before moves into that account. An unknown, used or expired code is a 404.
     */
    suspend fun restore(code: String) = switchAccount("/api/session/restore", mapOf("code" to code))

    /** Emails a 6-digit sign-in code. */
    suspend fun sendEmailCode(email: String) {
        request("POST", "/api/auth/email/code", jsonObject(mapOf("email" to email)))
    }

    /** Signs in with an emailed code. A wrong, used or expired code is a 404. */
    suspend fun signIn(email: String, code: String) =
        switchAccount("/api/auth/email", mapOf("email" to email, "code" to code))

    /** Signs this device out of its account; it starts afresh with a new, empty one. */
    suspend fun signOut() {
        account().devices.firstOrNull { it.current }?.let { removeDevice(it.id) }
        tokenLock.withLock { tokens.token = null }
    }

    private fun jsonObject(fields: Map<String, String?>): String =
        kotlinx.serialization.json.JsonObject(fields.mapValues { kotlinx.serialization.json.JsonPrimitive(it.value) }).toString()

    /**
     * Sends the device's token along to an endpoint that answers with another account's
     * session (what this device held merges into it), and keeps that session.
     */
    private suspend fun switchAccount(path: String, fields: Map<String, String>) = tokenLock.withLock {
        val body = jsonObject(fields + ("device" to deviceName))
        // The current token goes first so this device's progress merges; a rejected one is dropped.
        val text = try {
            request("POST", path, body, tokens.token)
        } catch (e: ApiException) {
            if (e.status != 401 || tokens.token == null) throw e
            tokens.token = null
            request("POST", path, body)
        }
        tokens.token = json.decodeFromString<Session>(text).token
    }

    /** Signs another device out of this account. */
    suspend fun removeDevice(id: String) {
        authed("DELETE", "/api/account/devices/${encode(id)}")
    }

    /** Deletes the account and everything in it. This device starts over with a new one. */
    suspend fun deleteAccount() {
        authed("DELETE", "/api/account")
        tokenLock.withLock { tokens.token = null }
    }

    private fun area(id: String) = "area=${encode(id)}"

    /** The device's time zone, so event days begin and end at local midnight. */
    private fun tz() = "tz=${encode(java.util.TimeZone.getDefault().id)}"

    private fun encode(value: String) = URLEncoder.encode(value, "UTF-8").replace("+", "%20")

    private suspend fun token(): String = tokenLock.withLock {
        tokens.token ?: json.decodeFromString<Session>(
            request("POST", "/api/session", json.encodeToString(NewSessionBody.serializer(), NewSessionBody(deviceName))),
        ).token
            .also { tokens.token = it }
    }

    private suspend fun authed(method: String, path: String, body: String? = null): String {
        val sent = token()
        return try {
            request(method, path, body, sent)
        } catch (e: ApiException) {
            // The server no longer knows this token (its database was reset): start over once.
            // A token stored meanwhile, by a restore, is newer and stays.
            if (e.status != 401) throw e
            tokenLock.withLock { if (tokens.token == sent) tokens.token = null }
            request(method, path, body, token())
        }
    }

    private suspend fun request(
        method: String,
        path: String,
        body: String? = null,
        token: String? = null,
    ): String = withContext(Dispatchers.IO) {
        val connection = try {
            (URL(baseUrl.trimEnd('/') + path).openConnection() as HttpURLConnection).apply {
                requestMethod = method
                connectTimeout = 10_000
                readTimeout = 100_000
                token?.let { setRequestProperty("Authorization", "Bearer $it") }
                if (body != null) {
                    doOutput = true
                    setRequestProperty("Content-Type", "application/json")
                    outputStream.use { it.write(body.toByteArray()) }
                }
            }
        } catch (e: IOException) {
            throw ApiException(0, "Cannot reach the server.")
        }
        try {
            val status = connection.responseCode
            if (status in 200..299) {
                if (status == 204) "" else connection.inputStream.bufferedReader().use { it.readText() }
            } else {
                val text = connection.errorStream?.bufferedReader()?.use { it.readText() }.orEmpty()
                val detail = runCatching { json.decodeFromString<ErrorBody>(text) }.getOrNull()
                throw ApiException(status, detail?.error ?: "Request failed ($status)", detail?.code)
            }
        } catch (e: IOException) {
            throw ApiException(0, "Cannot reach the server.")
        } finally {
            connection.disconnect()
        }
    }
}
