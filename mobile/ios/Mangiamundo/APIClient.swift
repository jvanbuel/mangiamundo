import Foundation

struct APIError: LocalizedError {
    let status: Int
    let message: String

    var errorDescription: String? { message }
}

/// Talks to the Mangiamundo backend. Accounts are anonymous: the device asks for a
/// token once and keeps it in UserDefaults.
actor APIClient {
    private let baseURL: String
    private let session: URLSession
    private let decoder: JSONDecoder
    private let defaults = UserDefaults.standard
    private static let tokenKey = "mangiamundo.token"
    private var pendingToken: Task<String, Error>?

    init(baseURL: String) {
        self.baseURL = baseURL.hasSuffix("/") ? String(baseURL.dropLast()) : baseURL
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

    /// Turns on news of food not tasted yet showing up in the area, sent through APNs to
    /// this device token. Call again when the token or the area changes. Fails with status
    /// 404 when the server can't send to iOS.
    func registerDevice(token: String, areaID: String, sandbox: Bool) async throws {
        let body: [String: Any] = [
            "platform": "ios", "token": token, "sandbox": sandbox,
            "area": areaID, "timezone": TimeZone.current.identifier,
        ]
        _ = try await authed("PUT", "/api/push/device", body: try JSONSerialization.data(withJSONObject: body))
    }

    /// Turns that news off again.
    func unregisterDevice(token: String) async throws {
        let data = try JSONSerialization.data(withJSONObject: ["token": token])
        _ = try await authed("POST", "/api/push/device/delete", body: data)
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
            let session: Session = try decode(await request("POST", "/api/session"))
            return session.token
        }
        pendingToken = task
        defer { pendingToken = nil }
        let token = try await task.value
        defaults.set(token, forKey: Self.tokenKey)
        return token
    }

    private func authed(_ method: String, _ path: String, body: Data? = nil) async throws -> Data {
        do {
            return try await request(method, path, body: body, token: try await token())
        } catch let error as APIError where error.status == 401 {
            // The server no longer knows this token (its database was reset): start over once.
            defaults.removeObject(forKey: Self.tokenKey)
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
            struct ErrorBody: Decodable { let error: String? }
            let detail = (try? decoder.decode(ErrorBody.self, from: data))?.error
            throw APIError(status: status, message: detail ?? "Request failed (\(status))")
        }
        return data
    }
}
