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

    func catalog() async throws -> Catalog {
        try decode(await request("GET", "/api/countries"))
    }

    func discover(lat: Double, lng: Double, radiusKm: Int) async throws -> Discover {
        try decode(await authed("GET", "/api/discover?\(area(lat, lng, radiusKm))"))
    }

    func places(lat: Double, lng: Double, radiusKm: Int, country: String) async throws -> [Place] {
        try decode(await authed("GET", "/api/places?\(area(lat, lng, radiusKm))&country=\(country)"))
    }

    /// `here` is the device position, sent so the server can mark the visit as verified.
    func checkIn(placeID: String, here: (lat: Double, lng: Double)?) async throws -> VisitResult {
        var body: [String: Double] = [:]
        if let here {
            body = ["lat": here.lat, "lng": here.lng]
        }
        let data = try JSONSerialization.data(withJSONObject: body)
        return try decode(await authed("PUT", "/api/visits/\(encode(placeID))", body: data))
    }

    func undoCheckIn(placeID: String) async throws {
        _ = try await authed("DELETE", "/api/visits/\(encode(placeID))")
    }

    func geocode(_ query: String) async throws -> [GeoResult] {
        try decode(await request("GET", "/api/geocode?q=\(encode(query))"))
    }

    func reverseGeocode(lat: Double, lng: Double) async throws -> String? {
        struct Result: Decodable { let name: String? }
        let result: Result = try decode(await request("GET", "/api/geocode/reverse?lat=\(lat)&lng=\(lng)"))
        return result.name
    }

    // MARK: - Plumbing

    private func area(_ lat: Double, _ lng: Double, _ radiusKm: Int) -> String {
        "lat=\(lat)&lng=\(lng)&radius_km=\(radiusKm)"
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
