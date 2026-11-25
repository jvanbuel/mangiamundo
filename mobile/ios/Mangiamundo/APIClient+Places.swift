import Foundation

/// One place: its visits, reports, additions and refreshes from OpenStreetMap.
extension APIClient {
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

    /// "This place has closed": hides it for this user, and for everyone after a second report.
    /// With an area, the answer has the progress there without the place.
    @discardableResult
    func reportClosed(placeID: String, areaID: String? = nil) async throws -> ClosedResult {
        let query = areaID.map { "\(area($0))&" } ?? ""
        return try decode(await write("PUT", "/api/places/\(encode(placeID))/closed?\(query)\(timeZone())"))
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
}
