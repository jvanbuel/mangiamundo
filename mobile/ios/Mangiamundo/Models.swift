import Foundation

struct Country: Decodable {
    let code: String
    let name: String
    let continent: String
    let flag: String
}

struct Catalog: Decodable {
    let countries: [Country]
}

enum Status {
    case available, discovered, completed
}

struct CountryProgress: Decodable, Identifiable {
    let code: String
    let continent: String
    let places: Int
    let visited: Int

    var id: String { code }

    var status: Status {
        if visited == 0 { return .available }
        return visited >= places ? .completed : .discovered
    }
}

struct GroupProgress: Decodable, Identifiable {
    let code: String
    let name: String
    let available: Int
    let discovered: Int
    let completed: Int

    var id: String { code }
}

struct Badge: Decodable, Identifiable {
    let id: String
    let icon: String
    let title: String
    let description: String
    let progress: Int
    let target: Int
    let earned: Bool
}

struct Profile: Decodable {
    let xp: Int
    let level: Int
    let levelXp: Int
    let nextLevelXp: Int
    let visits: Int
    let countries: Int
}

struct Discover: Decodable {
    let countries: [CountryProgress]
    let continents: [GroupProgress]
    let world: GroupProgress
    let places: Int
    let placesVisited: Int
    let badges: [Badge]
    let profile: Profile
}

struct Place: Decodable, Identifiable {
    let id: String
    let name: String
    let lat: Double
    let lng: Double
    let address: String?
    let url: String?
    let countries: [String]
    let distanceM: Int
    var visitedAt: Int?
    var verified: Bool

    var visited: Bool { visitedAt != nil }
}

struct GeoResult: Decodable, Identifiable {
    let name: String
    let lat: Double
    let lng: Double

    var id: String { "\(name)|\(lat)|\(lng)" }
}

struct VisitResult: Decodable {
    let verified: Bool
    let profile: Profile
}

/// The place the user is exploring: their GPS position or a searched location.
struct SavedLocation: Codable, Equatable {
    var lat: Double
    var lng: Double
    var name: String?
    var fromGPS: Bool
}
