import Foundation

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
