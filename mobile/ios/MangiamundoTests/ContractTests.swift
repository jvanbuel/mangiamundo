import XCTest
@testable import Mangiamundo

/// Real responses, written by the backend's `writes_contract_fixtures` test.
/// The simulator reads them straight from the repository.
final class ContractTests: XCTestCase {
    private func decode<T: Decodable>(_ type: T.Type, _ name: String) throws -> T {
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .appendingPathComponent("../../../contract/\(name).json")
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return try decoder.decode(T.self, from: Data(contentsOf: url))
    }

    func testDecodesContractFixtures() throws {
        XCTAssertGreaterThan(try decode(Catalog.self, "countries").countries.count, 200)
        let discover = try decode(Discover.self, "discover")
        XCTAssertEqual(discover.placesVisited, 2)
        XCTAssertFalse(discover.events.isEmpty)
        XCTAssertEqual(discover.profile.streak, 1)
        XCTAssertTrue(discover.quests?.quests.contains { $0.id == "check-ins" && $0.done } ?? false)
        XCTAssertTrue(discover.badges.contains { $0.kind == .streak })
        let places = try decode([Place].self, "places")
        XCTAssertEqual(places.filter(\.visited).count, 2)
        XCTAssertEqual(places.filter(\.verified).count, 1)
        let celebrations = try decode([Celebration].self, "celebrations")
        XCTAssertTrue(celebrations.allSatisfy { !$0.nearby.isEmpty })
        XCTAssertTrue(places.contains { $0.hours?.status != nil })
        let visit = try decode(VisitResult.self, "visit")
        XCTAssertTrue(visit.visit.verified)
        XCTAssertEqual(visit.progress?.placesVisited, 1)
        XCTAssertEqual(visit.progress?.profile.level, 2)
        XCTAssertEqual(visit.unlocked.map(\.kind), [.country, .badge, .level])
        XCTAssertEqual(visit.unlocked[0].country, "AZ")
        XCTAssertEqual(visit.unlocked[1].badge, .visits)
        XCTAssertEqual(visit.unlocked[2].level, 2)
        let catalog = try decode(Catalog.self, "countries")
        XCTAssertEqual(visit.unlocked[0].announcement(catalog: catalog), "🇦🇿 Azerbaijan: gold. 2 more for diamond.")
        XCTAssertEqual(visit.unlocked[2].announcement(catalog: catalog), "Level 2. 175 XP so far.")
        let removed = try decode(VisitRemoved.self, "visit-removed")
        XCTAssertTrue(removed.visits.isEmpty)
        XCTAssertEqual(removed.progress?.placesVisited, 1)
        XCTAssertEqual(removed.profile.visits, 1)
        let place = try decode(Place.self, "place")
        XCTAssertEqual(place.visits.count, 1)
        XCTAssertTrue(place.verified)
        XCTAssertEqual(place.hours?.week?.count, 7)
        XCTAssertEqual(place.hours?.week?.filter(\.today).count, 1)
        XCTAssertTrue(places.allSatisfy { $0.hours?.week == nil })
        XCTAssertFalse(try decode([Area].self, "areas").isEmpty)
        XCTAssertEqual(try decode(Located.self, "located").options.count, 2)
        let unknown = try decode([UnknownPlace].self, "unknown-places")
        XCTAssertEqual(unknown.first?.distanceM, 96)
        let added = try decode(AddedPlace.self, "added")
        XCTAssertEqual(added.place.name, "Kiwi")
        XCTAssertEqual(added.reward, Reward(xp: 380, bonus: .continent, areaName: "Leuven"))
        let additions = try decode([Addition].self, "additions")
        XCTAssertEqual(additions[0].status, .pending)
        XCTAssertFalse(additions[0].isNews)
        XCTAssertEqual(additions[0].id, "user-0123456789abcdef0123456789abcdef")
        let account = try decode(Account.self, "account")
        XCTAssertEqual(account.devices.count, 2)
        XCTAssertEqual(account.devices.first { !$0.current }?.name, "iPhone")
        XCTAssertNil(account.devices.first { $0.current }?.name)
        XCTAssertNotNil(account.recoveryCodeCreatedAt)
        XCTAssertEqual(try decode(LinkCode.self, "link-code").code, "ABCD-EFGH")
    }

    /// The answers that are not lists of places: one fixture each.
    func testDecodesTheSmallerAnswers() throws {
        let health = try decode(Health.self, "health")
        XCTAssertEqual(health.commit, "0123abc")
        XCTAssertFalse(health.version?.isEmpty ?? true)
        XCTAssertEqual(try decode(Session.self, "session").token, "0123456789abcdef0123456789abcdef")
        XCTAssertEqual(try decode(RecoveryCode.self, "recovery-code").code, "ABCD-EFGH-JKLM-NPQR-STUV")
        let link = try decode(ShareLink.self, "share-link")
        XCTAssertEqual(link.url, "https://mangiamundo.test/p/abcd2345ef")
        XCTAssertEqual(link.imageUrl, link.url + "/card.png")
        XCTAssertTrue(link.text.contains("Leuven"))
        XCTAssertEqual(
            try decode(AreaProgress?.self, "area-progress"),
            AreaProgress(percent: 15, text: "Looking for restaurants")
        )
    }

    func testDecodesASuggestionAndWhatARefreshFound() throws {
        let suggestion = try decode(Suggestion.self, "suggestion")
        XCTAssertEqual(suggestion.place.countries, [suggestion.country])
        XCTAssertTrue(suggestion.place.visits.isEmpty)
        XCTAssertEqual(suggestion.place.hours?.status?.open, true)
        let refreshed = try decode(RefreshResult.self, "refresh")
        XCTAssertEqual(refreshed.status, .updated)
        XCTAssertEqual(refreshed.place?.name, "Lalibela")
        XCTAssertEqual(refreshed.place?.hours?.week?.count, 7)
        let nearby = try decode(NearbyResult.self, "nearby")
        XCTAssertEqual(nearby.added.map(\.name), ["Restaurant Inti"])
        XCTAssertEqual(nearby.added.first?.osmUrl, "https://www.openstreetmap.org/node/43")
        XCTAssertEqual(nearby.matched, 1)
    }

    func testDecodesRefreshResults() throws {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        let gone = try decoder.decode(RefreshResult.self, from: Data(#"{"status":"gone","place":null}"#.utf8))
        XCTAssertEqual(gone.status, .gone)
        XCTAssertNil(gone.place)
        let updated = try decoder.decode(
            RefreshResult.self,
            from: Data(#"{"status":"updated","place":{"id":"osm-node-1","name":"Lalibela","lat":50.88,"lng":4.7,"address":null,"url":null,"cuisine":"ethiopian","hours":null,"countries":["ET"],"distance_m":0,"visits":[],"osm_url":null}}"#.utf8)
        )
        XCTAssertEqual(updated.place?.name, "Lalibela")
        XCTAssertEqual(try decoder.decode(RefreshResult.self, from: Data(#"{"status":"later","place":null}"#.utf8)).status, .unknown)
        let nearby = try decoder.decode(NearbyResult.self, from: Data(#"{"added":[],"matched":1}"#.utf8))
        XCTAssertEqual(nearby.matched, 1)
    }

    func testRewardsReadAsSentences() throws {
        let catalog = try decode(Catalog.self, "countries")
        XCTAssertEqual(
            Reward(xp: 380, bonus: .continent, areaName: "Leuven").pendingText(country: "NZ", catalog: catalog),
            "+380 XP once it is on OpenStreetMap: Oceania food would be new in Leuven."
        )
        XCTAssertEqual(Reward(xp: 30, bonus: .none, areaName: nil).pendingText(country: "NZ", catalog: catalog), "+30 XP once it is on OpenStreetMap.")
        let addition = try decode([Addition].self, "additions")[0]
        XCTAssertEqual(addition.announcement(catalog: catalog), "🇳🇿 Kiwi is on the map. +380 XP, and Oceania food is new in Leuven!")
    }

    func testToleratesWhatANewerServerSends() throws {
        let discover = try decode(Discover.self, "discover-newer")
        XCTAssertEqual(discover.badges[0].kind, .unknown)
        XCTAssertEqual(discover.events[0].status, .unknown)
        XCTAssertEqual(discover.countries[0].status, .discovered)
    }
}
