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

/// What a badge rewards. The app picks the badge's icon and colour from it.
enum BadgeKind: String, Decodable {
    case visits, countries, completed, continent, world, everything
}

struct Badge: Decodable, Identifiable {
    let id: String
    let kind: BadgeKind
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

enum EventStatus: String, Decodable {
    case upcoming, active, ended
}

/// A time-limited challenge, like "Japan month", and how far the user got.
struct EventProgress: Decodable, Identifiable {
    let id: String
    let title: String
    let description: String
    let countries: [String]
    /// First and last day, inclusive, as YYYY-MM-DD.
    let starts: String
    let ends: String
    let target: Int
    let progress: Int
    let earned: Bool
    let status: EventStatus
    let placesNearby: Int
}

struct NearbyCountry: Decodable {
    let code: String
    let places: Int
}

/// Notification text, written by the server so every app says the same thing.
struct Reminder: Decodable {
    let title: String
    let body: String
}

struct Celebration: Decodable, Identifiable {
    let id: String
    let title: String
    let description: String
    let countries: [String]
    let starts: String
    let ends: String
    let approximate: Bool
    /// The celebration's countries that have places in the area.
    let nearby: [NearbyCountry]
    let reminder: Reminder
}

struct Discover: Decodable {
    let countries: [CountryProgress]
    let continents: [GroupProgress]
    let world: GroupProgress
    let places: Int
    let placesVisited: Int
    let badges: [Badge]
    let profile: Profile
    let events: [EventProgress]
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
