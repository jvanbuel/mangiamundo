import Foundation

struct APIError: LocalizedError {
    let status: Int
    let message: String
    /// The server's error code, such as `nothing_open`.
    var code: String? = nil

    var errorDescription: String? { message }
}

/// Talks to the Mangiamundo backend. Accounts are anonymous: the device asks for a token
/// with the first thing it changes (a check-in, an added place) and keeps it in the
/// Keychain. What it only reads, it reads without one.
actor APIClient {
    let baseURL: String
    private let session: URLSession
    private let decoder: JSONDecoder
    let tokens: TokenStore
    private var pendingToken: Task<String, Error>?
    /// How this device shows up in the account's device list, such as "iPhone".
    let deviceName: String?
    /// Gets a value each time the server turns out not to know this device's token any
    /// more, after the token is dropped: the device was signed out from another one, or its
    /// account was deleted. No new account is made then; that waits for the next change.
    nonisolated let signedOut: AsyncStream<Void>
    private let signedOutContinuation: AsyncStream<Void>.Continuation

    init(
        baseURL: String,
        deviceName: String? = nil,
        tokens: TokenStore = TokenStore(),
        configuration: URLSessionConfiguration = .default
    ) {
        self.baseURL = baseURL.hasSuffix("/") ? String(baseURL.dropLast()) : baseURL
        self.deviceName = deviceName
        self.tokens = tokens
        configuration.timeoutIntervalForRequest = 100
        session = URLSession(configuration: configuration)
        decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        let (stream, continuation) = AsyncStream<Void>.makeStream()
        signedOut = stream
        signedOutContinuation = continuation
    }

    // MARK: - Plumbing

    func area(_ id: String) -> String {
        "area=\(encode(id))"
    }

    /// The device's time zone, so event days begin and end at local midnight.
    func timeZone() -> String {
        "tz=\(encode(TimeZone.current.identifier))"
    }

    func encode(_ value: String) -> String {
        var allowed = CharacterSet.alphanumerics
        allowed.insert(charactersIn: "-._~")
        return value.addingPercentEncoding(withAllowedCharacters: allowed) ?? value
    }

    func decode<T: Decodable>(_ data: Data) throws -> T {
        try decoder.decode(T.self, from: data)
    }

    /// This device's token; its account is made now when there is none yet.
    private func token() async throws -> String {
        if let stored = tokens.token { return stored }
        if let pendingToken { return try await pendingToken.value }
        let task = Task<String, Error> {
            var body: [String: Any] = [:]
            body["device"] = deviceName
            let data = try JSONSerialization.data(withJSONObject: body)
            let session: Session = try decode(await request("POST", "/api/session", body: data))
            return session.token
        }
        pendingToken = task
        defer { pendingToken = nil }
        let token = try await task.value
        // A restore may have stored a token meanwhile: keep that one.
        if let stored = tokens.token { return stored }
        tokens.token = token
        return token
    }

    /// The server answered 401 to `sent`: this device was signed out. The token goes and
    /// the app is told. A token stored meanwhile (by a restore) is newer: it stays.
    private func dropToken(_ sent: String) {
        guard tokens.token == sent else { return }
        tokens.token = nil
        signedOutContinuation.yield()
    }

    /// Reads something the server also shows to nobody in particular: with this device's
    /// token when it has one, without an account otherwise. Reading never makes an account.
    func read(_ path: String) async throws -> Data {
        let sent = tokens.token
        do {
            return try await request("GET", path, token: sent)
        } catch let error as APIError where error.status == 401 && sent != nil {
            if let sent { dropToken(sent) }
            // With the token a restore stored meanwhile, or as nobody.
            return try await request("GET", path, token: tokens.token)
        }
    }

    /// Reads something of the account's; nil when this device has no account (any more).
    func mine(_ path: String) async throws -> Data? {
        guard let sent = tokens.token else { return nil }
        do {
            return try await request("GET", path, token: sent)
        } catch let error as APIError where error.status == 401 {
            dropToken(sent)
            guard let newer = tokens.token else { return nil }
            return try await request("GET", path, token: newer)
        }
    }

    /// Changes something, as this device's account, which is made now when there is none
    /// yet. A device that was signed out is told so and the change fails: doing it again
    /// starts a new account, knowingly.
    func write(_ method: String, _ path: String, body: Data? = nil) async throws -> Data {
        let sent = try await token()
        do {
            return try await request(method, path, body: body, token: sent)
        } catch let error as APIError where error.status == 401 {
            dropToken(sent)
            guard let newer = tokens.token else {
                throw APIError(
                    status: 401,
                    message: "This device was signed out. Sign in again, or repeat this to start afresh here.",
                    code: "unauthorized"
                )
            }
            return try await request(method, path, body: body, token: newer)
        }
    }

    /// Like `write`, for a change that only matters to an account that exists: nothing
    /// happens without one.
    func writeMine(_ method: String, _ path: String, body: Data? = nil) async throws {
        guard let sent = tokens.token else { return }
        do {
            _ = try await request(method, path, body: body, token: sent)
        } catch let error as APIError where error.status == 401 {
            dropToken(sent)
        }
    }

    func request(_ method: String, _ path: String, body: Data? = nil, token: String? = nil) async throws -> Data {
        guard let url = URL(string: baseURL + path) else {
            throw APIError(status: 0, message: "Invalid server address.")
        }
        var request = URLRequest(url: url)
        request.httpMethod = method
        if let token {
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        }
        if let body {
            request.httpBody = body
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        }
        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await session.data(for: request)
        } catch {
            throw APIError(status: 0, message: "Cannot reach the server.")
        }
        let status = (response as? HTTPURLResponse)?.statusCode ?? 0
        guard (200..<300).contains(status) else {
            struct ErrorBody: Decodable { let error: String?; let code: String? }
            let detail = try? decoder.decode(ErrorBody.self, from: data)
            throw APIError(status: status, message: detail?.error ?? "Request failed (\(status))", code: detail?.code)
        }
        return data
    }
}
