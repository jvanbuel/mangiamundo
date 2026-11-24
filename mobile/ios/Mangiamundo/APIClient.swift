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
    private let tokens: TokenStore
    private var pendingToken: Task<String, Error>?
    /// How this device shows up in the account's device list, such as "iPhone".
    private let deviceName: String?
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

    func health() async throws -> Health {
        try decode(await request("GET", "/api/health"))
    }

    func catalog() async throws -> Catalog {
        try decode(await request("GET", "/api/countries"))
    }

    func discover(areaID: String) async throws -> Discover {
        try decode(await read("/api/discover?\(area(areaID))&\(timeZone())"))
    }

    /// Distances are measured from `from` when given, otherwise from the area's centre.
    func places(areaID: String, country: String, from: (lat: Double, lng: Double)?) async throws -> [Place] {
        let position = from.map { "&lat=\($0.lat)&lng=\($0.lng)" } ?? ""
        return try decode(await read("/api/places?\(area(areaID))&country=\(country)\(position)"))
    }

    /// A random unvisited place to try, open now unless `open` is false, skipping `exclude`.
    func suggestion(areaID: String, from: (lat: Double, lng: Double)?, exclude: [String], open: Bool) async throws -> Suggestion {
        let position = from.map { "&lat=\($0.lat)&lng=\($0.lng)" } ?? ""
        let skip = exclude.map(encode).joined(separator: ",")
        return try decode(await read("/api/suggestion?\(area(areaID))&open=\(open)&exclude=\(skip)\(position)"))
    }

    func celebrations(areaID: String, days: Int = 60) async throws -> [Celebration] {
        try decode(await request("GET", "/api/celebrations?\(area(areaID))&days=\(days)&\(timeZone())"))
    }

    /// The reminders that become due in the area in the coming `days`, soonest first, for this user.
    func reminders(areaID: String, days: Int = 60) async throws -> [Reminder] {
        try decode(await read("/api/reminders?\(area(areaID))&days=\(days)&\(timeZone())"))
    }

    /// One place with all of the user's visits. The distance is measured from `from` when given.
    func place(id: String, from: (lat: Double, lng: Double)?) async throws -> Place {
        let position = from.map { "?lat=\($0.lat)&lng=\($0.lng)" } ?? ""
        return try decode(await read("/api/places/\(encode(id))\(position)"))
    }

    /// Checks in, and answers with the progress in the area afterwards and what the check-in
    /// unlocked. `here` is the device position, sent so the server can mark the visit as verified.
    func checkIn(placeID: String, areaID: String, here: (lat: Double, lng: Double)?) async throws -> VisitResult {
        var body: [String: [String: Double]] = [:]
        if let here {
            body = ["position": ["lat": here.lat, "lng": here.lng]]
        }
        let data = try JSONSerialization.data(withJSONObject: body)
        return try decode(await write("POST", "/api/places/\(encode(placeID))/visits?\(area(areaID))&\(timeZone())", body: data))
    }

    /// Takes a check-in back. The answer has the visits left at the place and the progress in the area.
    func removeVisit(id: Int, areaID: String) async throws -> VisitRemoved {
        try decode(await write("DELETE", "/api/visits/\(id)?\(area(areaID))&\(timeZone())"))
    }

    /// Makes (or moves to this area) the public page of the user's progress.
    func shareProgress(areaID: String) async throws -> ShareLink {
        try decode(await write("POST", "/api/share?\(area(areaID))"))
    }

    /// Turns the public page off.
    func unshareProgress() async throws {
        try await writeMine("DELETE", "/api/share")
    }

    /// A public page for one check-in.
    func shareVisit(id: Int) async throws -> ShareLink {
        try decode(await write("POST", "/api/visits/\(id)/share"))
    }

    /// The share card; nil when it can't be loaded, so the link is shared alone.
    func card(_ link: ShareLink) async -> Data? {
        guard let url = URL(string: link.imageUrl),
              let (data, response) = try? await URLSession.shared.data(from: url),
              (response as? HTTPURLResponse)?.statusCode == 200 else { return nil }
        return data
    }

    /// "This place has closed": hides it for this user, and for everyone after a second report.
    /// With an area, the answer has the progress there without the place.
    @discardableResult
    func reportClosed(placeID: String, areaID: String? = nil) async throws -> ClosedResult {
        let query = areaID.map { "\(area($0))&" } ?? ""
        return try decode(await write("PUT", "/api/places/\(encode(placeID))/closed?\(query)\(timeZone())"))
    }

    /// How far loading the area has come; nil when it isn't loading.
    func areaProgress(areaID: String) async throws -> AreaProgress? {
        try decode(await request("GET", "/api/areas/\(encode(areaID))/progress"))
    }

    /// Adds a restaurant OpenStreetMap lacks. Only this user sees it until OSM has it.
    /// `note: false` when the user adds it to OSM themselves, so no note asks mappers to.
    func addPlace(name: String, country: String, lat: Double, lng: Double, address: String?, website: String?, note: Bool) async throws -> AddedPlace {
        var body: [String: Any] = ["name": name, "country": country, "lat": lat, "lng": lng, "note": note]
        body["address"] = address
        body["website"] = website
        let data = try JSONSerialization.data(withJSONObject: body)
        return try decode(await write("POST", "/api/places", body: data))
    }

    /// Restaurants in the area OpenStreetMap gives no cuisine, nearest to `from` first.
    func unknownPlaces(areaID: String, from: (lat: Double, lng: Double)?) async throws -> [UnknownPlace] {
        let position = from.map { "&lat=\($0.lat)&lng=\($0.lng)" } ?? ""
        return try decode(await read("/api/unknown-places?\(area(areaID))\(position)"))
    }

    /// "It serves food from …": like adding a place, only this user sees it until OSM has the cuisine.
    func suggestCuisine(placeID: String, country: String) async throws -> AddedPlace {
        let data = try JSONSerialization.data(withJSONObject: ["country": country])
        return try decode(await write("POST", "/api/unknown-places/\(encode(placeID))/cuisine", body: data))
    }

    /// "It serves no particular country's food": hides it for this user, and for everyone after two.
    func noCuisine(placeID: String) async throws {
        _ = try await write("PUT", "/api/unknown-places/\(encode(placeID))/no-cuisine")
    }

    /// Reads a place again from OpenStreetMap, for an edit made there since the daily import.
    func refreshPlace(id: String, from: (lat: Double, lng: Double)?) async throws -> RefreshResult {
        let position = from.map { "?lat=\($0.lat)&lng=\($0.lng)" } ?? ""
        return try decode(await write("POST", "/api/places/\(encode(id))/refresh\(position)"))
    }

    /// Asks OpenStreetMap for restaurants within 150 m of a point, for one just mapped there.
    func refreshNearby(lat: Double, lng: Double) async throws -> NearbyResult {
        try decode(await write("POST", "/api/places/nearby/refresh?lat=\(lat)&lng=\(lng)"))
    }

    /// The restaurants this user added, newest first, with their rewards.
    func additions() async throws -> [Addition] {
        guard let data = try await mine("/api/additions") else { return [] }
        return try decode(data)
    }

    /// The user was told about these confirmed additions, by `id` (at most 100).
    func markAdditionsSeen(ids: [String]) async throws {
        let data = try JSONSerialization.data(withJSONObject: ["ids": ids])
        try await writeMine("PUT", "/api/additions/seen", body: data)
    }

    /// Cities and postal codes matching what someone typed.
    func searchAreas(_ query: String) async throws -> [Area] {
        try decode(await request("GET", "/api/areas?q=\(encode(query))"))
    }

    /// Suggestions while someone types, nearest first when `near` is known.
    func suggestAreas(_ query: String, near: (lat: Double, lng: Double)?) async throws -> [Area] {
        let point = near.map { "&lat=\($0.lat)&lng=\($0.lng)" } ?? ""
        return try decode(await request("GET", "/api/areas/suggest?q=\(encode(query))\(point)"))
    }

    /// The city and postal code a point lies in.
    func locateArea(lat: Double, lng: Double) async throws -> Located {
        try decode(await request("GET", "/api/areas/locate?lat=\(lat)&lng=\(lng)"))
    }

    // MARK: - Account

    /// Whether this device has an account yet; it gets one with its first check-in or other change.
    var hasAccount: Bool { tokens.token != nil }

    /// The account's recovery code status and the devices signed in to it. A device without
    /// an account has an empty one, with no devices: nothing is on the server until its
    /// first change.
    func account() async throws -> Account {
        guard let data = try await mine("/api/account") else { return .none }
        return try decode(data)
    }

    /// A new recovery code, shown once. The previous one stops working.
    func newRecoveryCode() async throws -> String {
        let made: RecoveryCode = try decode(await write("POST", "/api/account/recovery-code"))
        return made.code
    }

    /// A code that signs another device in to this account, once, within 10 minutes.
    func newLinkCode() async throws -> LinkCode {
        try decode(await write("POST", "/api/account/link-code"))
    }

    /// Signs this device in to the account of a recovery or link code. What this device did
    /// before moves into that account. An unknown, used or expired code is a 404.
    func restore(code: String) async throws {
        try await switchAccount("/api/session/restore", ["code": code])
    }

    /// Emails a 6-digit sign-in code.
    func sendEmailCode(_ email: String) async throws {
        let data = try JSONSerialization.data(withJSONObject: ["email": email])
        _ = try await request("POST", "/api/auth/email/code", body: data)
    }

    /// Signs in with an emailed code. A wrong, used or expired code is a 404.
    func signIn(email: String, code: String) async throws {
        try await switchAccount("/api/auth/email", ["email": email, "code": code])
    }

    /// Signs in with an ID token from Sign in with Apple.
    func signInWithApple(idToken: String) async throws {
        try await switchAccount("/api/auth/apple", ["id_token": idToken])
    }

    /// Signs in with an ID token from Google's sign-in.
    func signInWithGoogle(idToken: String) async throws {
        try await switchAccount("/api/auth/google", ["id_token": idToken])
    }

    /// Signs this device out of its account; it starts afresh, without one.
    func signOut() async throws {
        if let me = try await account().devices.first(where: \.current) {
            try await removeDevice(id: me.id)
        }
        tokens.token = nil
    }

    /// Sends the device's token along to an endpoint that answers with another account's
    /// session (what this device held merges into it), and keeps that session.
    private func switchAccount(_ path: String, _ fields: [String: String]) async throws {
        var body = fields
        body["device"] = deviceName
        let data = try JSONSerialization.data(withJSONObject: body)
        let current = tokens.token
        let session: Session
        do {
            session = try decode(await request("POST", path, body: data, token: current))
        } catch let error as APIError where error.status == 401 && current != nil {
            // This device was signed out elsewhere: drop that token and go on without it.
            if tokens.token == current { tokens.token = nil }
            session = try decode(await request("POST", path, body: data))
        }
        tokens.token = session.token
    }

    /// Signs another device out of this account.
    func removeDevice(id: String) async throws {
        try await writeMine("DELETE", "/api/account/devices/\(encode(id))")
    }

    /// Deletes the account and everything in it. This device starts over without one.
    func deleteAccount() async throws {
        try await writeMine("DELETE", "/api/account")
        tokens.token = nil
    }

    // MARK: - Plumbing

    private func area(_ id: String) -> String {
        "area=\(encode(id))"
    }

    /// The device's time zone, so event days begin and end at local midnight.
    private func timeZone() -> String {
        "tz=\(encode(TimeZone.current.identifier))"
    }

    private func encode(_ value: String) -> String {
        var allowed = CharacterSet.alphanumerics
        allowed.insert(charactersIn: "-._~")
        return value.addingPercentEncoding(withAllowedCharacters: allowed) ?? value
    }

    private func decode<T: Decodable>(_ data: Data) throws -> T {
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
    private func read(_ path: String) async throws -> Data {
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
    private func mine(_ path: String) async throws -> Data? {
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
    private func write(_ method: String, _ path: String, body: Data? = nil) async throws -> Data {
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
    private func writeMine(_ method: String, _ path: String, body: Data? = nil) async throws {
        guard let sent = tokens.token else { return }
        do {
            _ = try await request(method, path, body: body, token: sent)
        } catch let error as APIError where error.status == 401 {
            dropToken(sent)
        }
    }

    private func request(_ method: String, _ path: String, body: Data? = nil, token: String? = nil) async throws -> Data {
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
