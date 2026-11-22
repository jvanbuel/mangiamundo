import Foundation

struct APIError: LocalizedError {
    let status: Int
    let message: String
    /// The server's error code, such as `nothing_open`.
    var code: String? = nil

    var errorDescription: String? { message }
}

/// Talks to the Mangiamundo backend. Accounts are anonymous: the device asks for a
/// token once and keeps it in UserDefaults.
actor APIClient {
    let baseURL: String
    private let session: URLSession
    private let decoder: JSONDecoder
    private let defaults = UserDefaults.standard
    private static let tokenKey = "mangiamundo.token"
    private var pendingToken: Task<String, Error>?
    /// How this device shows up in the account's device list, such as "iPhone".
    private let deviceName: String?

    init(baseURL: String, deviceName: String? = nil) {
        self.baseURL = baseURL.hasSuffix("/") ? String(baseURL.dropLast()) : baseURL
        self.deviceName = deviceName
        let configuration = URLSessionConfiguration.default
        configuration.timeoutIntervalForRequest = 100
        session = URLSession(configuration: configuration)
        decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
    }

    func health() async throws -> Health {
        try decode(await request("GET", "/api/health"))
    }

    func catalog() async throws -> Catalog {
        try decode(await request("GET", "/api/countries"))
    }

    func discover(areaID: String) async throws -> Discover {
        try decode(await authed("GET", "/api/discover?\(area(areaID))&\(timeZone())"))
    }

    /// Distances are measured from `from` when given, otherwise from the area's centre.
    func places(areaID: String, country: String, from: (lat: Double, lng: Double)?) async throws -> [Place] {
        let position = from.map { "&lat=\($0.lat)&lng=\($0.lng)" } ?? ""
        return try decode(await authed("GET", "/api/places?\(area(areaID))&country=\(country)\(position)"))
    }

    /// A random unvisited place to try, open now unless `open` is false, skipping `exclude`.
    func suggestion(areaID: String, from: (lat: Double, lng: Double)?, exclude: [String], open: Bool) async throws -> Suggestion {
        let position = from.map { "&lat=\($0.lat)&lng=\($0.lng)" } ?? ""
        let skip = exclude.map(encode).joined(separator: ",")
        return try decode(await authed("GET", "/api/suggestion?\(area(areaID))&open=\(open)&exclude=\(skip)\(position)"))
    }

    func celebrations(areaID: String, days: Int = 60) async throws -> [Celebration] {
        try decode(await request("GET", "/api/celebrations?\(area(areaID))&days=\(days)&\(timeZone())"))
    }

    /// One place with all of the user's visits. The distance is measured from `from` when given.
    func place(id: String, from: (lat: Double, lng: Double)?) async throws -> Place {
        let position = from.map { "?lat=\($0.lat)&lng=\($0.lng)" } ?? ""
        return try decode(await authed("GET", "/api/places/\(encode(id))\(position)"))
    }

    /// `here` is the device position, sent so the server can mark the visit as verified.
    func checkIn(placeID: String, here: (lat: Double, lng: Double)?) async throws -> VisitResult {
        var body: [String: [String: Double]] = [:]
        if let here {
            body = ["position": ["lat": here.lat, "lng": here.lng]]
        }
        let data = try JSONSerialization.data(withJSONObject: body)
        return try decode(await authed("POST", "/api/places/\(encode(placeID))/visits?\(timeZone())", body: data))
    }

    func removeVisit(id: Int) async throws {
        _ = try await authed("DELETE", "/api/visits/\(id)")
    }

    /// Makes (or moves to this area) the public page of the user's progress.
    func shareProgress(areaID: String) async throws -> ShareLink {
        try decode(await authed("POST", "/api/share?\(area(areaID))"))
    }

    /// Turns the public page off.
    func unshareProgress() async throws {
        _ = try await authed("DELETE", "/api/share")
    }

    /// A public page for one check-in.
    func shareVisit(id: Int) async throws -> ShareLink {
        try decode(await authed("POST", "/api/visits/\(id)/share"))
    }

    /// The share card; nil when it can't be loaded, so the link is shared alone.
    func card(_ link: ShareLink) async -> Data? {
        guard let url = URL(string: link.imageUrl),
              let (data, response) = try? await URLSession.shared.data(from: url),
              (response as? HTTPURLResponse)?.statusCode == 200 else { return nil }
        return data
    }

    /// "This place has closed": hides it for this user, and for everyone after a second report.
    func reportClosed(placeID: String) async throws {
        _ = try await authed("PUT", "/api/places/\(encode(placeID))/closed")
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
        return try decode(await authed("POST", "/api/places", body: data))
    }

    /// Reads a place again from OpenStreetMap, for an edit made there since the daily import.
    func refreshPlace(id: String, from: (lat: Double, lng: Double)?) async throws -> RefreshResult {
        let position = from.map { "?lat=\($0.lat)&lng=\($0.lng)" } ?? ""
        return try decode(await authed("POST", "/api/places/\(encode(id))/refresh\(position)"))
    }

    /// Asks OpenStreetMap for restaurants within 150 m of a point, for one just mapped there.
    func refreshNearby(lat: Double, lng: Double) async throws -> NearbyResult {
        try decode(await authed("POST", "/api/places/nearby/refresh?lat=\(lat)&lng=\(lng)"))
    }

    /// The restaurants this user added, newest first, with their rewards.
    func additions() async throws -> [Addition] {
        try decode(await authed("GET", "/api/additions"))
    }

    /// The user was told about these confirmed additions, by `id` (at most 100).
    func markAdditionsSeen(ids: [String]) async throws {
        let data = try JSONSerialization.data(withJSONObject: ["ids": ids])
        _ = try await authed("PUT", "/api/additions/seen", body: data)
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

    /// The account's recovery code status and the devices signed in to it.
    func account() async throws -> Account {
        try decode(await authed("GET", "/api/account"))
    }

    /// A new recovery code, shown once. The previous one stops working.
    func newRecoveryCode() async throws -> String {
        struct RecoveryCode: Decodable { let code: String }
        let made: RecoveryCode = try decode(await authed("POST", "/api/account/recovery-code"))
        return made.code
    }

    /// A code that signs another device in to this account, once, within 10 minutes.
    func newLinkCode() async throws -> LinkCode {
        try decode(await authed("POST", "/api/account/link-code"))
    }

    /// Signs this device in to the account of a recovery or link code. What this device did
    /// before moves into that account. An unknown, used or expired code is a 404.
    func restore(code: String) async throws {
        var body: [String: Any] = ["code": code]
        body["device"] = deviceName
        let data = try JSONSerialization.data(withJSONObject: body)
        struct Session: Decodable { let token: String }
        let current = defaults.string(forKey: Self.tokenKey)
        let session: Session
        do {
            // The current token lets what this device did merge into the restored account.
            session = try decode(await request("POST", "/api/session/restore", body: data, token: current))
        } catch let error as APIError where error.status == 401 && current != nil {
            // This device was signed out elsewhere: drop that token and restore without it.
            if defaults.string(forKey: Self.tokenKey) == current { defaults.removeObject(forKey: Self.tokenKey) }
            session = try decode(await request("POST", "/api/session/restore", body: data))
        }
        defaults.set(session.token, forKey: Self.tokenKey)
    }

    /// Signs another device out of this account.
    func removeDevice(id: String) async throws {
        _ = try await authed("DELETE", "/api/account/devices/\(encode(id))")
    }

    /// Deletes the account and everything in it. This device starts over with a new one.
    func deleteAccount() async throws {
        _ = try await authed("DELETE", "/api/account")
        defaults.removeObject(forKey: Self.tokenKey)
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

    private func token() async throws -> String {
        if let stored = defaults.string(forKey: Self.tokenKey) { return stored }
        if let pendingToken { return try await pendingToken.value }
        let task = Task<String, Error> {
            struct Session: Decodable { let token: String }
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
        if let stored = defaults.string(forKey: Self.tokenKey) { return stored }
        defaults.set(token, forKey: Self.tokenKey)
        return token
    }

    private func authed(_ method: String, _ path: String, body: Data? = nil) async throws -> Data {
        let sent = try await token()
        do {
            return try await request(method, path, body: body, token: sent)
        } catch let error as APIError where error.status == 401 {
            // The server no longer knows this token (its database was reset): start over once.
            // A token stored meanwhile (by a restore) is newer: keep it.
            if defaults.string(forKey: Self.tokenKey) == sent { defaults.removeObject(forKey: Self.tokenKey) }
            return try await request(method, path, body: body, token: try await token())
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
