import Foundation

/// A city (every postal code in it) or a single postal code.
struct Area: Codable, Equatable, Identifiable {
    /// Opaque, e.g. "osm-relation-196311".
    let id: String
    /// "Leuven", or "3000 Leuven" for a postal code.
    let name: String
    let kind: AreaKind
    /// Where it is, to tell same-named areas apart: "Flemish Brabant, Belgium".
    let detail: String?
    let lat: Double
    let lng: Double
}

enum AreaKind: String, Codable {
    case city
    case postalCode = "postal_code"

    /// A kind this app doesn't know yet reads as a city instead of failing.
    init(from decoder: Decoder) throws {
        self = AreaKind(rawValue: try decoder.singleValueContainer().decode(String.self)) ?? .city
    }
}

/// How far loading an area's restaurants has come, from `/api/areas/{id}/progress`.
struct AreaProgress: Decodable, Equatable {
    /// 0 to 100.
    let percent: Int
    /// What is happening, e.g. "Looking for restaurants".
    let text: String
}

/// The areas a point lies in.
struct Located: Decodable {
    let city: Area?
    let postalCode: Area?

    var options: [Area] { [city, postalCode].compactMap { $0 } }
}

/// The area the user is exploring: a city or a postal code, found by GPS or a search.
struct SavedLocation: Codable, Equatable {
    var area: Area
    /// Where the map pin goes: the GPS position, or the area's centre.
    var lat: Double
    var lng: Double
    var fromGPS: Bool
    /// The city and postal code found at the GPS position, to switch between.
    var options: [Area] = []
}
