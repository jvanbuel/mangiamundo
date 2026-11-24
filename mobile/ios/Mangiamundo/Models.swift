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

/// The account's recovery code status and the devices signed in to it.
struct Account: Decodable {
    let createdAt: Int
    /// When the current recovery code was made; nil when there is none yet.
    let recoveryCodeCreatedAt: Int?
    /// Most recently used first.
    let devices: [Device]
    /// The public page of the user's progress, like `/p/abcd2345ef`; nil when it's off.
    let publicProfile: String?
    /// The ways to sign in, oldest first; empty for an account that lives on its devices.
    let identities: [Identity]
}

/// A way to sign in: Sign in with Apple, Google or an email address.
struct Identity: Decodable, Hashable {
    /// "apple", "google" or "email".
    let provider: String
    let email: String?
}

/// A public link to share, with its card image and a sentence to go with it.
struct ShareLink: Decodable {
    let url: String
    let imageUrl: String
    let text: String
}

/// A device signed in to the account.
struct Device: Decodable, Identifiable {
    let id: String
    /// What the app called itself; nil for devices from before devices had names.
    let name: String?
    let createdAt: Int
    let lastUsedAt: Int
    /// The device this app runs on.
    let current: Bool
}

/// A code that signs another device in to the account, once, until `expiresAt` (Unix seconds).
struct LinkCode: Decodable, Equatable {
    let code: String
    let expiresAt: Int
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

/// From `POST /api/places/{id}/visits`.
struct VisitResult: Decodable {
    let visit: Visit
    let profile: Profile
    /// The progress in the area after the check-in; nil when no area was sent.
    let progress: Discover?
    /// What the check-in unlocked, in the order to announce it.
    let unlocked: [Unlocked]
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

/// What a restaurant the user adds brings to their area, decided when they add it.
enum Bonus: String, Decodable {
    /// Also what a value from a newer server reads as.
    case none
    /// Its country had no places in the area yet.
    case country
    /// Its continent had no places in the area yet.
    case continent

    init(from decoder: Decoder) throws {
        self = Bonus(rawValue: try decoder.singleValueContainer().decode(String.self)) ?? .none
    }
}

/// XP for an added restaurant, granted once OpenStreetMap has it.
struct Reward: Decodable, Equatable {
    let xp: Int
    let bonus: Bonus
    let areaName: String?

    /// "+130 XP once it is on OpenStreetMap: Ethiopia would be new in Leuven."
    func pendingText(country: String, catalog: Catalog?) -> String {
        let extra = bonusText(bonus, country: country, areaName: areaName, tense: "would be", catalog: catalog)
        return "+\(xp) XP once it is on OpenStreetMap" + (extra.map { ": \($0)." } ?? ".")
    }
}

/// A named restaurant OpenStreetMap gives no cuisine, so it isn't on the map yet.
struct UnknownPlace: Decodable, Identifiable, Equatable {
    let id: String
    let name: String
    let lat: Double
    let lng: Double
    let address: String?
    let distanceM: Int
}

/// `POST /api/places` sends the place with its reward next to its fields.
struct AddedPlace: Decodable {
    let place: Place
    let reward: Reward

    private enum CodingKeys: String, CodingKey { case reward }

    init(from decoder: Decoder) throws {
        place = try Place(from: decoder)
        reward = try decoder.container(keyedBy: CodingKeys.self).decode(Reward.self, forKey: .reward)
    }
}

enum AdditionStatus: String, Decodable {
    case pending, confirmed
    /// On OpenStreetMap, but someone else added it first.
    case duplicate
    /// A status from a newer server.
    case unknown

    init(from decoder: Decoder) throws {
        self = AdditionStatus(rawValue: try decoder.singleValueContainer().decode(String.self)) ?? .unknown
    }
}

/// A restaurant the user added, from `/api/additions`.
struct Addition: Decodable {
    /// The addition's own id, which never changes; `placeId` becomes the OSM place's.
    let id: String
    let placeId: String
    let name: String
    let country: String
    let areaName: String?
    let bonus: Bonus
    let xp: Int
    let status: AdditionStatus
    let submittedAt: Int
    let confirmedAt: Int?
    let seen: Bool

    /// Confirmed since the user was last told.
    var isNews: Bool { !seen && (status == .confirmed || status == .duplicate) }

    /// "Lalibela is on the map. +130 XP, and Ethiopia is new in Leuven!"
    func announcement(catalog: Catalog?) -> String {
        let flag = catalog?.countries.first(where: { $0.code == country }).map { "\($0.flag) " } ?? ""
        if status == .duplicate {
            return "\(flag)\(name) is on the map. Someone added it before you, so it earned no XP."
        }
        if let extra = bonusText(bonus, country: country, areaName: areaName, tense: "is", catalog: catalog) {
            return "\(flag)\(name) is on the map. +\(xp) XP, and \(extra)!"
        }
        return "\(flag)\(name) is on the map. +\(xp) XP."
    }
}

/// What an addition brings to its area, as "Ethiopia is new in Leuven"; nil without a bonus.
func bonusText(_ bonus: Bonus, country: String, areaName: String?, tense: String, catalog: Catalog?) -> String? {
    guard let info = catalog?.countries.first(where: { $0.code == country }) else { return nil }
    let place = areaName.map { " in \($0)" } ?? " here"
    switch bonus {
    case .none:
        return nil
    case .country:
        return "\(info.name) \(tense) new\(place)"
    case .continent:
        guard let continent = catalog?.continents.first(where: { $0.code == info.continent }) else {
            return "\(info.name) \(tense) new\(place)"
        }
        return "\(continent.name) food \(tense) new\(place)"
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
