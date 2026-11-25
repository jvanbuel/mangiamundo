import Security
import XCTest
@testable import Mangiamundo

/// Stands in for the Keychain; `failing` makes it refuse, as a locked phone does.
private final class FakeSecrets: SecretStore, @unchecked Sendable {
    var value: String?
    var failing = false

    func read() throws -> String? {
        if failing { throw KeychainError(status: errSecInteractionNotAllowed) }
        return value
    }

    func write(_ value: String?) throws {
        if failing { throw KeychainError(status: errSecInteractionNotAllowed) }
        self.value = value
    }
}

/// Where the session token is kept, and how one from before the Keychain gets there.
final class TokenStoreTests: XCTestCase {
    private var suite: String!
    private var defaults: UserDefaults!
    private var secrets: FakeSecrets!

    override func setUp() {
        suite = "mangiamundo.tests.\(UUID().uuidString)"
        defaults = UserDefaults(suiteName: suite)
        secrets = FakeSecrets()
    }

    override func tearDown() {
        defaults.removePersistentDomain(forName: suite)
    }

    func testATokenFromUserDefaultsMovesToTheKeychainOnce() {
        defaults.set("old-token", forKey: TokenStore.legacyKey)

        let store = TokenStore(secrets: secrets, legacy: defaults)

        XCTAssertEqual(store.token, "old-token")
        XCTAssertEqual(secrets.value, "old-token")
        XCTAssertNil(defaults.string(forKey: TokenStore.legacyKey))
        // The next launch finds nothing left to move, and the token where it belongs.
        secrets.value = "newer-token"
        XCTAssertEqual(TokenStore(secrets: secrets, legacy: defaults).token, "newer-token")
    }

    func testTokensAreKeptInTheKeychainOnly() {
        let store = TokenStore(secrets: secrets, legacy: defaults)
        XCTAssertNil(store.token)

        store.token = "token"
        XCTAssertEqual(secrets.value, "token")
        XCTAssertNil(defaults.string(forKey: TokenStore.legacyKey))
        XCTAssertEqual(store.token, "token")

        store.token = nil
        XCTAssertNil(secrets.value)
        XCTAssertNil(store.token)
    }

    func testATokenWaitsInUserDefaultsWhileTheKeychainRefuses() {
        defaults.set("old-token", forKey: TokenStore.legacyKey)
        secrets.failing = true

        let store = TokenStore(secrets: secrets, legacy: defaults)

        // Not moved, and not lost.
        XCTAssertEqual(store.token, "old-token")
        XCTAssertEqual(defaults.string(forKey: TokenStore.legacyKey), "old-token")
        store.token = "new-token"
        XCTAssertEqual(store.token, "new-token")
        XCTAssertNil(secrets.value)

        // Once the Keychain takes it, at the next launch, it moves.
        secrets.failing = false
        let later = TokenStore(secrets: secrets, legacy: defaults)
        XCTAssertEqual(later.token, "new-token")
        XCTAssertEqual(secrets.value, "new-token")
        XCTAssertNil(defaults.string(forKey: TokenStore.legacyKey))
    }

    func testSigningOutHoldsWhileTheKeychainRefuses() {
        secrets.value = "token"
        let store = TokenStore(secrets: secrets, legacy: defaults)
        secrets.failing = true

        store.token = nil

        // The Keychain still has the token; the device is signed out all the same.
        XCTAssertNil(store.token)
        secrets.failing = false
        XCTAssertNil(store.token)
        XCTAssertNil(TokenStore(secrets: secrets, legacy: defaults).token)
        XCTAssertNil(secrets.value)
        XCTAssertNil(defaults.string(forKey: TokenStore.legacyKey))
    }
}

/// A server that makes sessions and answers 401 to a token it doesn't know.
private final class StubServer: URLProtocol {
    private static let lock = NSLock()
    private static var known: Set<String> = []
    private static var seen: [String] = []

    /// Every request so far: "GET /api/unknown-places token-1".
    static var requests: [String] { lock.withLock { seen } }

    static func reset() {
        lock.withLock {
            known = []
            seen = []
        }
    }

    /// As when the device is signed out from another one. The requests start over too.
    static func forgetTokens() {
        reset()
    }

    private static func answer(_ request: URLRequest) -> (Int, String) {
        lock.withLock {
            let method = request.httpMethod ?? "GET"
            let path = request.url?.path ?? ""
            let token = request.value(forHTTPHeaderField: "Authorization")?.replacingOccurrences(of: "Bearer ", with: "")
            seen.append([method, path, token].compactMap { $0 }.joined(separator: " "))
            let unauthorized = (401, #"{"error":"missing or invalid session token","code":"unauthorized"}"#)
            if let token, !known.contains(token) { return unauthorized }
            if path == "/api/session" {
                let made = "token-\(known.count + 1)"
                known.insert(made)
                return (200, #"{"token":"\#(made)"}"#)
            }
            // What needs an account answers 401 without a token too.
            if token == nil, method != "GET" || path == "/api/account" || path == "/api/additions" { return unauthorized }
            if path == "/api/account" {
                return (200, #"{"created_at":1,"devices":[{"id":"d","created_at":1,"last_used_at":1,"current":true}],"identities":[]}"#)
            }
            if path.hasSuffix("/closed") { return (200, #"{"progress":null}"#) }
            return method == "GET" ? (200, "[]") : (204, "")
        }
    }

    override class func canInit(with request: URLRequest) -> Bool { true }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        let (status, body) = Self.answer(request)
        guard let url = request.url,
              let response = HTTPURLResponse(url: url, statusCode: status, httpVersion: "HTTP/1.1", headerFields: nil) else {
            client?.urlProtocol(self, didFailWithError: URLError(.badURL))
            return
        }
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: Data(body.utf8))
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}
}

/// When the device gets its account, and what happens once the server no longer knows it.
final class APIClientAccountTests: XCTestCase {
    private var suite: String!
    private var defaults: UserDefaults!
    private var tokens: TokenStore!
    private var client: APIClient!

    override func setUp() {
        StubServer.reset()
        suite = "mangiamundo.tests.\(UUID().uuidString)"
        defaults = UserDefaults(suiteName: suite)
        tokens = TokenStore(secrets: FakeSecrets(), legacy: defaults)
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [StubServer.self]
        client = APIClient(baseURL: "http://stub.test", deviceName: "Test", tokens: tokens, configuration: configuration)
    }

    override func tearDown() {
        defaults.removePersistentDomain(forName: suite)
    }

    /// Whether the client said the device was signed out. Ends the stream: ask once, last.
    private func saidSignedOut() async -> Bool {
        let events = client.signedOut
        return await withTaskGroup(of: Bool.self) { group in
            group.addTask {
                for await _ in events { return true }
                return false
            }
            group.addTask {
                try? await Task.sleep(for: .milliseconds(300))
                return false
            }
            let first = await group.next() ?? false
            group.cancelAll()
            return first
        }
    }

    func testReadingMakesNoAccountAndTheFirstChangeDoes() async throws {
        _ = try await client.unknownPlaces(areaID: "a", from: nil)
        let none = try await client.additions()
        XCTAssertTrue(none.isEmpty)
        try await client.markAdditionsSeen(ids: ["a"])
        let empty = try await client.account()
        XCTAssertTrue(empty.devices.isEmpty)
        try await client.deleteAccount()
        XCTAssertEqual(StubServer.requests, ["GET /api/unknown-places"])
        XCTAssertNil(tokens.token)
        let before = await client.hasAccount
        XCTAssertFalse(before)

        try await client.reportClosed(placeID: "p")
        XCTAssertEqual(tokens.token, "token-1")
        XCTAssertEqual(Array(StubServer.requests.dropFirst()), ["POST /api/session", "PUT /api/places/p/closed token-1"])
        // From here on the account is sent along, also when reading.
        _ = try await client.unknownPlaces(areaID: "a", from: nil)
        XCTAssertEqual(StubServer.requests.last, "GET /api/unknown-places token-1")
        let account = try await client.account()
        XCTAssertEqual(account.devices.count, 1)
        XCTAssertEqual(StubServer.requests.filter { $0 == "POST /api/session" }.count, 1)
        let signedOut = await saidSignedOut()
        XCTAssertFalse(signedOut)
    }

    func testASignedOutDeviceIsToldAndReadsOnWithoutAnAccount() async throws {
        try await client.reportClosed(placeID: "p")
        StubServer.forgetTokens()

        _ = try await client.unknownPlaces(areaID: "a", from: nil)
        XCTAssertEqual(StubServer.requests, ["GET /api/unknown-places token-1", "GET /api/unknown-places"])
        XCTAssertNil(tokens.token)
        // No new account is made unasked, whatever is read next.
        let account = try await client.account()
        XCTAssertTrue(account.devices.isEmpty)
        _ = try await client.additions()
        _ = try await client.unknownPlaces(areaID: "a", from: nil)
        XCTAssertFalse(StubServer.requests.contains("POST /api/session"))
        let signedOut = await saidSignedOut()
        XCTAssertTrue(signedOut)
    }

    func testAChangeByASignedOutDeviceFailsOnceAndTheNextStartsAfresh() async throws {
        try await client.reportClosed(placeID: "p")
        StubServer.forgetTokens()

        do {
            try await client.reportClosed(placeID: "p")
            XCTFail("The change went through")
        } catch let error as APIError {
            XCTAssertEqual(error.status, 401)
            XCTAssertTrue(error.message.contains("signed out"), error.message)
        }
        XCTAssertEqual(StubServer.requests, ["PUT /api/places/p/closed token-1"])
        XCTAssertNil(tokens.token)

        try await client.reportClosed(placeID: "p")
        // The server's first token since it forgot the others.
        XCTAssertEqual(tokens.token, "token-1")
        XCTAssertEqual(StubServer.requests.filter { $0 == "POST /api/session" }.count, 1)
        let signedOut = await saidSignedOut()
        XCTAssertTrue(signedOut)
    }

    func testAChangeThatNeedsAnAccountOnlyTellsASignedOutDevice() async throws {
        try await client.reportClosed(placeID: "p")
        StubServer.forgetTokens()

        try await client.markAdditionsSeen(ids: ["a"])
        XCTAssertNil(tokens.token)
        XCTAssertFalse(StubServer.requests.contains("POST /api/session"))
        let signedOut = await saidSignedOut()
        XCTAssertTrue(signedOut)
    }
}
