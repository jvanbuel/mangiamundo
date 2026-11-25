package app.mangiamundo.core

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL

/**
 * The requests behind [ApiClient]: the device's token, made with the first change and dropped
 * when the server no longer knows it (then `onSignedOut` is called), and the HTTP calls.
 */
internal class ApiTransport(
    private val baseUrl: String,
    private val tokens: TokenStore,
    private val deviceName: String?,
    private val onSignedOut: () -> Unit,
    private val json: Json,
) {
    private val tokenLock = Mutex()

    /** Drops the device's token: it starts afresh, without an account. */
    suspend fun dropToken() {
        tokenLock.withLock { tokens.token = null }
    }

    /**
     * Sends the device's token along to an endpoint that answers with another account's
     * session (what this device held merges into it), and keeps that session.
     */
    suspend fun switchAccount(path: String, fields: Map<String, String>) = tokenLock.withLock {
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

    private suspend fun token(): String = tokenLock.withLock {
        tokens.token ?: json.decodeFromString<Session>(
            request("POST", "/api/session", json.encodeToString(NewSessionBody.serializer(), NewSessionBody(deviceName))),
        ).token
            .also { tokens.token = it }
    }

    /**
     * The server answered 401 to `sent`: this device was signed out. The token goes and the
     * app is told. A token stored meanwhile, by a restore, is newer and stays.
     */
    private suspend fun signedOut(sent: String) {
        val dropped = tokenLock.withLock { (tokens.token == sent).also { if (it) tokens.token = null } }
        if (dropped) onSignedOut()
    }

    /**
     * Reads something the server also shows to nobody in particular: with this device's token
     * when it has one, without an account otherwise. Reading never makes an account.
     */
    suspend fun read(path: String): String {
        val sent = tokens.token
        return try {
            request("GET", path, token = sent)
        } catch (e: ApiException) {
            if (e.status != 401 || sent == null) throw e
            signedOut(sent)
            // With the token a restore stored meanwhile, or as nobody.
            request("GET", path, token = tokens.token)
        }
    }

    /** Reads something of the account's; null when this device has no account (any more). */
    suspend fun mine(path: String): String? {
        val sent = tokens.token ?: return null
        return try {
            request("GET", path, token = sent)
        } catch (e: ApiException) {
            if (e.status != 401) throw e
            signedOut(sent)
            tokens.token?.let { request("GET", path, token = it) }
        }
    }

    /**
     * Changes something, as this device's account, which is made now when there is none yet.
     * A device that was signed out is told so and the change fails: doing it again starts a
     * new account, knowingly.
     */
    suspend fun write(method: String, path: String, body: String? = null): String {
        val sent = token()
        return try {
            request(method, path, body, sent)
        } catch (e: ApiException) {
            if (e.status != 401) throw e
            signedOut(sent)
            val newer = tokens.token
                ?: throw ApiException(401, "This device was signed out. Sign in again, or repeat this to start afresh here.", "unauthorized")
            request(method, path, body, newer)
        }
    }

    /** Like [write], for a change that only matters to an account that exists: nothing happens without one. */
    suspend fun writeMine(method: String, path: String, body: String? = null) {
        val sent = tokens.token ?: return
        try {
            request(method, path, body, sent)
        } catch (e: ApiException) {
            if (e.status != 401) throw e
            signedOut(sent)
        }
    }

    suspend fun request(
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

/** A JSON object of string fields. */
internal fun jsonObject(fields: Map<String, String?>): String =
    kotlinx.serialization.json.JsonObject(fields.mapValues { kotlinx.serialization.json.JsonPrimitive(it.value) }).toString()
