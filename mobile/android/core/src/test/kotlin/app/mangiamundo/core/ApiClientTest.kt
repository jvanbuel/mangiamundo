package app.mangiamundo.core

import com.sun.net.httpserver.HttpServer
import kotlinx.coroutines.runBlocking
import java.net.InetSocketAddress
import kotlin.test.AfterTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** When the device gets its account, and what happens once the server no longer knows it. */
class ApiClientTest {
    /** The tokens the server knows, and every request it got: "GET /api/discover token-1". */
    private val known = mutableSetOf<String>()
    private val requests = mutableListOf<String>()
    private var signedOut = 0

    /** A server that makes sessions and answers 401 to a token it doesn't know. */
    private val server = HttpServer.create(InetSocketAddress("127.0.0.1", 0), 0).apply {
        createContext("/") { exchange ->
            val token = exchange.requestHeaders.getFirst("Authorization")?.removePrefix("Bearer ")
            val path = exchange.requestURI.path
            requests += listOfNotNull(exchange.requestMethod, path, token).joinToString(" ")
            val (status, body) = when {
                token != null && token !in known -> 401 to """{"error":"missing or invalid session token","code":"unauthorized"}"""
                path == "/api/session" -> 200 to """{"token":"${"token-${known.size + 1}".also { known += it }}"}"""
                // What needs an account answers 401 without a token too.
                token == null && (exchange.requestMethod != "GET" || path == "/api/account") -> 401 to """{"error":"x","code":"unauthorized"}"""
                path == "/api/account" -> 200 to """{"created_at":1,"devices":[{"id":"d","created_at":1,"last_used_at":1,"current":true}]}"""
                path == "/api/additions" -> 200 to "[]"
                path.endsWith("/closed") -> 200 to """{"progress":null}"""
                exchange.requestMethod == "GET" -> 200 to """{"id":"p","name":"P","lat":0,"lng":0,"distance_m":0,"visits":[]}"""
                else -> 204 to ""
            }
            val bytes = body.toByteArray()
            exchange.sendResponseHeaders(status, if (bytes.isEmpty()) -1 else bytes.size.toLong())
            if (bytes.isNotEmpty()) exchange.responseBody.use { it.write(bytes) }
            exchange.close()
        }
        start()
    }
    private val store = object : TokenStore { override var token: String? = null }
    private val client = ApiClient("http://127.0.0.1:${server.address.port}", store, "Test") { signedOut++ }

    @AfterTest
    fun stop() = server.stop(0)

    private fun sessionsMade() = requests.count { it == "POST /api/session" }

    @Test
    fun readingMakesNoAccountAndTheFirstChangeDoes() = runBlocking {
        client.place("p", null)
        client.additions()
        client.markAdditionsSeen(listOf("a"))
        assertEquals(0, client.account().devices.size)
        client.deleteAccount()
        assertEquals(listOf("GET /api/places/p"), requests)
        assertNull(store.token)
        assertFalse(client.hasAccount)

        client.reportClosed("p")
        assertEquals("token-1", store.token)
        assertEquals(listOf("POST /api/session", "PUT /api/places/p/closed token-1"), requests.drop(1))
        // From here on the account is sent along, also when reading.
        client.place("p", null)
        assertEquals("GET /api/places/p token-1", requests.last())
        assertEquals(1, client.account().devices.size)
        assertEquals(1, sessionsMade())
        assertEquals(0, signedOut)
    }

    @Test
    fun aSignedOutDeviceIsToldAndReadsOnWithoutAnAccount() = runBlocking {
        client.reportClosed("p")
        known.clear() // signed out from another device
        requests.clear()

        client.place("p", null)
        assertEquals(listOf("GET /api/places/p token-1", "GET /api/places/p"), requests)
        assertNull(store.token)
        assertEquals(1, signedOut)
        // No new account is made unasked, whatever is read next.
        assertEquals(0, client.account().devices.size)
        client.additions()
        client.place("p", null)
        assertEquals(0, sessionsMade())
        assertEquals(1, signedOut)
    }

    @Test
    fun aChangeBySignedOutDeviceFailsOnceAndTheNextStartsAfresh() = runBlocking {
        client.reportClosed("p")
        known.clear()
        requests.clear()

        val failure = assertFailsWith<ApiException> { client.reportClosed("p") }
        assertEquals(401, failure.status)
        assertTrue("signed out" in failure.message!!, failure.message)
        assertEquals(listOf("PUT /api/places/p/closed token-1"), requests)
        assertNull(store.token)
        assertEquals(1, signedOut)

        client.reportClosed("p")
        assertEquals("token-1", store.token) // the server's first token since it forgot the others
        assertEquals(1, sessionsMade())
        assertEquals(1, signedOut)
    }

    @Test
    fun aChangeThatNeedsAnAccountOnlyTellsASignedOutDevice() = runBlocking {
        client.reportClosed("p")
        known.clear()
        requests.clear()

        client.markAdditionsSeen(listOf("a"))
        assertNull(store.token)
        assertEquals(1, signedOut)
        assertEquals(0, sessionsMade())
    }
}
