import Foundation

struct Country: Decodable {
    let code: String
    let name: String
    let continent: String
    let flag: String
    /// Its food's OpenStreetMap `cuisine` value; only countries with one can be picked for a new place.
    var cuisine: String? = nil
}

/// From `/api/health`. An older server sends only `ok`.
struct Health: Decodable {
    let version: String?
    let commit: String?
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
    /// A kind from a newer server. Shown with a neutral medal.
    case unknown

    init(from decoder: Decoder) throws {
        self = BadgeKind(rawValue: try decoder.singleValueContainer().decode(String.self)) ?? .unknown
    }
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
    /// A status from a newer server. The event is hidden.
    case unknown

    init(from decoder: Decoder) throws {
        self = EventStatus(rawValue: try decoder.singleValueContainer().decode(String.self)) ?? .unknown
    }
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
    let cuisine: String?
    /// Nil when the hours are unknown.
    let hours: Hours?
    let countries: [String]
    let distanceM: Int
    /// Newest first. Lists of places carry only the latest visit; `APIClient.place` all of them.
    var visits: [Visit]
    /// The place's page on openstreetmap.org. Nil for places that don't come from OSM.
    var osmUrl: String? = nil

    /// Nil when the hours are unknown or can't be read.
    var openNow: Bool? { hours?.status?.open }
    /// Short status line, e.g. "Open until 22:00".
    var hoursText: String? { hours?.status?.text }
    var visited: Bool { !visits.isEmpty }
    /// The latest visit was checked in on the spot.
    var verified: Bool { visits.first?.verified ?? false }

    /// Whether the latest visit was today on `calendar`. Checking in again is for an earlier one.
    func visitedToday(now: Date = Date(), calendar: Calendar = .current) -> Bool {
        guard let latest = visits.first else { return false }
        return calendar.isDate(Date(timeIntervalSince1970: TimeInterval(latest.at)), inSameDayAs: now)
    }
}

struct Hours: Decodable {
    /// OSM opening_hours syntax.
    let raw: String
    /// Nil when `raw` can't be parsed.
    let status: HoursStatus?
    /// This week, Monday to Sunday. Only from `APIClient.place`; nil in lists.
    var week: [Day]? = nil
}

/// One day of a place's week.
struct Day: Decodable {
    /// "Monday".
    let name: String
    let today: Bool
    /// E.g. "11:30-14:00"; empty when closed all day.
    let open: [String]
}

struct HoursStatus: Decodable {
    let open: Bool
    let text: String
}

/// One check-in.
struct Visit: Decodable, Equatable, Identifiable {
    let id: Int
    /// Unix seconds.
    let at: Int
    /// Checked in within walking distance of the place.
    let verified: Bool
}

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

struct VisitResult: Decodable {
    let visit: Visit
    let profile: Profile
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

/// A location saved before areas existed: a point (and a radius, now ignored).
struct LegacyLocation: Decodable {
    var lat: Double
    var lng: Double
    var fromGPS: Bool
}
