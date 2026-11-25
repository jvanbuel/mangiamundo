import Foundation

/// The catalog, an area's progress and places, and finding areas.
extension APIClient {
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

    /// The places serving a country's food nearest to `from`, nearest first, whatever area they are in.
    func nearest(country: String, from: (lat: Double, lng: Double), limit: Int = 10) async throws -> [Place] {
        try decode(await read("/api/countries/\(country)/nearest?lat=\(from.lat)&lng=\(from.lng)&limit=\(limit)"))
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

    /// How far loading the area has come; nil when it isn't loading.
    func areaProgress(areaID: String) async throws -> AreaProgress? {
        try decode(await request("GET", "/api/areas/\(encode(areaID))/progress"))
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
}
