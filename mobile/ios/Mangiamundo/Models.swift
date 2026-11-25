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

struct Continent: Decodable {
    let code: String
    let name: String
}

struct Catalog: Decodable {
    let countries: [Country]
    let continents: [Continent]
}

/// How far the user got with a country's food: gold, platinum, then diamond.
enum Status: String, Decodable, CaseIterable {
    case available
    /// Gold.
    case discovered
    case platinum
    /// Diamond.
    case completed

    /// A status from a newer server reads as gold.
    init(from decoder: Decoder) throws {
        self = Status(rawValue: try decoder.singleValueContainer().decode(String.self)) ?? .discovered
    }

    /// What players call it.
    var label: String {
        switch self {
        case .available: "To discover"
        case .discovered: "Gold"
        case .platinum: "Platinum"
        case .completed: "Diamond"
        }
    }
}

struct CountryProgress: Decodable, Identifiable {
    let code: String
    let continent: String
    let places: Int
    let visited: Int
    /// Places to visit for diamond.
    let goal: Int
    let status: Status

    var id: String { code }
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
    case visits, countries, completed, continent, world, everything, pioneer, streak
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
    /// Restaurants the user added that OpenStreetMap now has. Missing from older servers.
    let added: Int?
    /// Months in a row with a check-in; a month without one yet doesn't break it until it's over.
    let streak: Int?
    let bestStreak: Int?
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

/// One of this month's quests.
struct Quest: Decodable, Identifiable {
    let id: String
    let title: String
    let description: String
    let progress: Int
    let target: Int
    let done: Bool
    let xp: Int
}

/// This month's quests, first to last day (YYYY-MM-DD, inclusive).
struct MonthlyQuests: Decodable {
    let month: String
    let starts: String
    let ends: String
    let quests: [Quest]
}

struct NearbyCountry: Decodable {
    let code: String
    let places: Int
}

/// A reminder the server plans, for the app to show as a local notification from `hour`
/// o'clock on `localDate` (YYYY-MM-DD) in the device's time zone.
struct Reminder: Decodable, Equatable {
    /// Stable for this reminder: scheduling it again under the same key replaces it.
    let key: String
    let localDate: String
    let hour: Int
    let title: String
    let body: String
    /// Where a tap leads, relative to the web app: `/?country=JP` or `/?tab=badges`.
    let url: String
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
    let quests: MonthlyQuests?
}
