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

    suspend fun celebrations(areaId: String, days: Int = 60): List<Celebration> =
        json.decodeFromString(request("GET", "/api/celebrations?${area(areaId)}&days=$days&${tz()}"))

    /** `here` is the device position, sent so the server can mark the visit as verified. */
    suspend fun checkIn(placeId: String, here: Pair<Double, Double>?): VisitResult {
        val body = json.encodeToString(CheckInBody.serializer(), CheckInBody(here?.first, here?.second))
        return json.decodeFromString(authed("PUT", "/api/visits/${encode(placeId)}?${tz()}", body))
    }

    suspend fun undoCheckIn(placeId: String) {
        authed("DELETE", "/api/visits/${encode(placeId)}")
    }

    /** Cities and postal codes matching what someone typed. */
    suspend fun searchAreas(query: String): List<Area> =
        json.decodeFromString(request("GET", "/api/areas?q=${encode(query)}"))

    /** The city and postal code a point lies in. */
    suspend fun locateArea(lat: Double, lng: Double): Located =
        json.decodeFromString(request("GET", "/api/areas/locate?lat=$lat&lng=$lng"))

    private fun area(id: String) = "area=${encode(id)}"

    /** The device's time zone, so event days begin and end at local midnight. */
    private fun tz() = "tz=${encode(java.util.TimeZone.getDefault().id)}"

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
