import Foundation

/// A place to try from `/api/suggestion`, and the country the globe lands on.
struct Suggestion: Decodable {
    let country: String
    let place: Place
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

/// From `POST /api/places/{id}/visits`.
struct VisitResult: Decodable {
    let visit: Visit
    let profile: Profile
    /// The progress in the area after the check-in; nil when no area was sent.
    let progress: Discover?
    /// What the check-in unlocked, in the order to announce it.
    let unlocked: [Unlocked]
}

struct VisitRemoved: Decodable {
    /// The visits left at the place, newest first.
    let visits: [Visit]
    let profile: Profile
    /// The progress in the area after the removal; nil when no area was sent.
    let progress: Discover?
}

struct ClosedResult: Decodable {
    /// The progress in the area without the place; nil when no area was sent.
    let progress: Discover?
}

enum UnlockedKind: String, Decodable {
    case country, badge, event, level
    /// A kind from a newer server. Announced with its words alone.
    case unknown

    init(from decoder: Decoder) throws {
        self = UnlockedKind(rawValue: try decoder.singleValueContainer().decode(String.self)) ?? .unknown
    }
}

/// One thing a check-in unlocked, with the server's words to announce it.
struct Unlocked: Decodable {
    let kind: UnlockedKind
    /// For a country: its code, to show its flag.
    let country: String?
    /// For a badge: which medal it is.
    let badge: BadgeKind?
    /// For a level: the level reached.
    let level: Int?
    let title: String
    let text: String

    /// "🇦🇿 Azerbaijan: gold. 2 more for diamond."
    func announcement(catalog: Catalog?) -> String {
        let flag = catalog?.countries.first(where: { $0.code == country }).map { "\($0.flag) " } ?? ""
        return text.isEmpty ? "\(flag)\(title)" : "\(flag)\(title). \(text)"
    }
}

/// What reading a place again from OpenStreetMap changed.
enum Refreshed: String, Decodable {
    case updated
    /// OpenStreetMap has the same as we do; a very recent edit may not have reached it yet.
    case unchanged
    /// No longer on OpenStreetMap as a restaurant: hidden now, visits kept.
    case gone
    /// A status from a newer server.
    case unknown

    init(from decoder: Decoder) throws {
        self = Refreshed(rawValue: try decoder.singleValueContainer().decode(String.self)) ?? .unknown
    }
}

/// From `POST /api/places/{id}/refresh`.
struct RefreshResult: Decodable {
    let status: Refreshed
    let place: Place?
}

/// From `POST /api/places/nearby/refresh`.
struct NearbyResult: Decodable {
    /// Places that weren't listed before, nearest first.
    let added: [Place]
    let matched: Int
}
