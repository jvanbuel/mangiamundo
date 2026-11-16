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

class ApiException(val status: Int, message: String) : Exception(message)

/** Where the anonymous account token of this device is kept. */
interface TokenStore {
    var token: String?
}

class ApiClient(private val baseUrl: String, private val tokens: TokenStore) {
    @PublishedApi
    internal val json = Json { ignoreUnknownKeys = true }
    private val tokenLock = Mutex()

    suspend fun catalog(): Catalog = json.decodeFromString(request("GET", "/api/countries"))

    suspend fun discover(lat: Double, lng: Double, radiusKm: Int): Discover =
        json.decodeFromString(authed("GET", "/api/discover?${area(lat, lng, radiusKm)}"))

    suspend fun places(lat: Double, lng: Double, radiusKm: Int, country: String): List<Place> =
        json.decodeFromString(authed("GET", "/api/places?${area(lat, lng, radiusKm)}&country=$country"))

    suspend fun celebrations(lat: Double, lng: Double, radiusKm: Int, days: Int = 60): List<Celebration> =
        json.decodeFromString(request("GET", "/api/celebrations?${area(lat, lng, radiusKm)}&days=$days"))

    /**
     * A new check-in; returning to a place counts again once the cooldown has passed.
     * `here` is the device position, sent so the server can mark the visit as verified.
     */
    suspend fun checkIn(placeId: String, here: Pair<Double, Double>?): VisitResult {
        val body = json.encodeToString(CheckInBody.serializer(), CheckInBody(here?.first, here?.second))
        return json.decodeFromString(authed("POST", "/api/visits/${encode(placeId)}", body))
    }

    /** Logs (or with null, clears) the signature dish eaten at a check-in. */
    suspend fun setDish(checkinId: Long, dish: String?): Profile {
        val body = json.encodeToString(DishBody.serializer(), DishBody(dish))
        // HttpURLConnection cannot send PATCH; the server takes PUT for the same update.
        return json.decodeFromString(authed("PUT", "/api/checkins/$checkinId", body))
    }

    suspend fun undoCheckIn(checkinId: Long) {
        authed("DELETE", "/api/checkins/$checkinId")
    }

    suspend fun recoveryCode(): String = json.decodeFromString<RecoveryCode>(authed("GET", "/api/session/recovery")).code

    /** Replaces the recovery code; the old one stops working. */
    suspend fun rotateRecoveryCode(): String =
        json.decodeFromString<RecoveryCode>(authed("POST", "/api/session/recovery")).code

    /**
     * Moves this device onto the passport behind a recovery code. What this device had
     * collected is merged in by the server. Returns how many check-ins came along.
     */
    suspend fun restore(code: String): Int = tokenLock.withLock {
        val body = json.encodeToString(RestoreBody.serializer(), RestoreBody(code))
        val restored = json.decodeFromString<Restored>(request("POST", "/api/session/restore", body, tokens.token))
        tokens.token = restored.token
        restored.merged
    }

    /** The share card as PNG bytes, drawn by the server. */
    suspend fun shareCard(lat: Double, lng: Double, radiusKm: Int, place: String?, dark: Boolean): ByteArray {
        val name = place?.let { "&place=${encode(it)}" }.orEmpty()
        val theme = if (dark) "dark" else "light"
        val path = "/api/share/card.png?${area(lat, lng, radiusKm)}$name&theme=$theme"
        return send("GET", path, null, token())
    }

    suspend fun geocode(query: String): List<GeoResult> =
        json.decodeFromString(request("GET", "/api/geocode?q=${encode(query)}"))

    suspend fun reverseGeocode(lat: Double, lng: Double): String? =
        json.decodeFromString<ReverseResult>(request("GET", "/api/geocode/reverse?lat=$lat&lng=$lng")).name

    private fun area(lat: Double, lng: Double, radiusKm: Int) = "lat=$lat&lng=$lng&radius_km=$radiusKm"

    private fun encode(value: String) = URLEncoder.encode(value, "UTF-8").replace("+", "%20")

    private suspend fun token(): String = tokenLock.withLock {
        tokens.token ?: json.decodeFromString<Session>(request("POST", "/api/session")).token
            .also { tokens.token = it }
    }

    private suspend fun authed(method: String, path: String, body: String? = null): String =
        try {
            request(method, path, body, token())
        } catch (e: ApiException) {
            // The server no longer knows this token (its database was reset): start over once.
            if (e.status != 401) throw e
            tokens.token = null
            request(method, path, body, token())
        }

    private suspend fun request(
        method: String,
        path: String,
        body: String? = null,
        token: String? = null,
    ): String = send(method, path, body, token).decodeToString()

    private suspend fun send(
        method: String,
        path: String,
        body: String?,
        token: String?,
    ): ByteArray = withContext(Dispatchers.IO) {
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
                if (status == 204) ByteArray(0) else connection.inputStream.use { it.readBytes() }
            } else {
                val text = connection.errorStream?.bufferedReader()?.use { it.readText() }.orEmpty()
                val detail = runCatching { json.decodeFromString<ErrorBody>(text).error }.getOrNull()
                throw ApiException(status, detail ?: "Request failed ($status)")
            }
        } catch (e: IOException) {
            throw ApiException(0, "Cannot reach the server.")
        } finally {
            connection.disconnect()
        }
    }
}
