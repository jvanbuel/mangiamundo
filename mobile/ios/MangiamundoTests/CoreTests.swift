import XCTest
@testable import Mangiamundo

final class CoreTests: XCTestCase {
    private var world: World!

    override func setUpWithError() throws {
        world = try XCTUnwrap(World.bundled())
    }

    func testSpinsLandOnTheMainland() throws {
        let france = try XCTUnwrap(world.countries.first { $0.code == "FR" }).landing()
        XCTAssertLessThan(abs(france.lng - 2.5), 4)
        XCTAssertLessThan(abs(france.lat - 46.5), 4)
        let belgium = try XCTUnwrap(world.countries.first { $0.code == "BE" }).landing()
        XCTAssertEqual(world.country(lng: belgium.lng, lat: belgium.lat)?.code, "BE")
        XCTAssertEqual(belgium.zoom, 4)
    }

    func testFindsCountriesByPosition() {
        XCTAssertEqual(world.country(lng: 4.70, lat: 50.88)?.code, "BE")
        XCTAssertEqual(world.country(lng: 139.65, lat: 35.68)?.code, "JP")
        // Eastern Russia is stored past the antimeridian (lng > 180).
        XCTAssertEqual(world.country(lng: -172, lat: 65.5)?.code, "RU")
        // Lesotho is a hole in South Africa.
        XCTAssertEqual(world.country(lng: 28.2, lat: -29.6)?.code, "LS")
        XCTAssertNil(world.country(lng: -30, lat: 30))
    }

    func testProjectionsRoundTrip() throws {
        let size = CGSize(width: 400, height: 600)
        for kind in [ProjectionKind.globe, .flat] {
            let projector = Projector(kind: kind, camera: Camera(lng: 4.7, lat: 50.9, zoom: 1.5), size: size)
            for (lng, lat) in [(4.7, 50.9), (-3.7, 40.4), (30.5, 50.4), (12.5, 41.9)] {
                let projected = projector.project(lng: lng, lat: lat)
                XCTAssertTrue(projected.visible)
                let back = try XCTUnwrap(projector.unproject(projected.point))
                XCTAssertEqual(back.lng, lng, accuracy: 0.01)
                XCTAssertEqual(back.lat, lat, accuracy: 0.01)
            }
            let centre = projector.project(lng: 4.7, lat: 50.9).point
            XCTAssertEqual(centre.x, 200, accuracy: 0.01)
            XCTAssertEqual(centre.y, 300, accuracy: 0.01)
        }
    }

    func testGlobeHidesTheFarSide() {
        let projector = Projector(kind: .globe, camera: Camera(lng: 0, lat: 0, zoom: 1), size: CGSize(width: 400, height: 400))
        let hidden = projector.project(lng: 170, lat: 10)
        XCTAssertFalse(hidden.visible)
        XCTAssertEqual(hypot(hidden.point.x - 200, hidden.point.y - 200), projector.radius, accuracy: 0.01)
        XCTAssertNil(projector.unproject(CGPoint(x: 2, y: 2)))
    }

    func testCameraStaysInBounds() {
        let camera = Camera(lng: 190, lat: 95, zoom: 50).normalized()
        XCTAssertEqual(camera.lng, -170, accuracy: 1e-9)
        XCTAssertEqual(camera.lat, Camera.maxLat)
        XCTAssertEqual(camera.zoom, Camera.maxZoom)
    }
}

final class AreaTests: XCTestCase {
    func testToleratesUnknownAreaKinds() throws {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        let newer = try decoder.decode(Area.self, from: Data(#"{"id":"x","name":"Mitte","kind":"district","detail":null,"lat":52.5,"lng":13.4}"#.utf8))
        XCTAssertEqual(newer.kind, .city)
    }

    func testSavedLocationsRoundTrip() throws {
        let leuven = Area(id: "demo-leuven", name: "Leuven", kind: .city, detail: "Belgium", lat: 50.88, lng: 4.70)
        let saved = SavedLocation(area: leuven, lat: 50.88, lng: 4.71, fromGPS: true, options: [leuven])
        let data = try JSONEncoder().encode(saved)
        XCTAssertEqual(try JSONDecoder().decode(SavedLocation.self, from: data), saved)
    }
}

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
        XCTAssertTrue(celebrations.allSatisfy { !$0.nearby.isEmpty && !$0.reminder.title.isEmpty })
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

final class DirectionsTests: XCTestCase {
    func testDirectionsURLs() {
        XCTAssertEqual(
            MapsApp.apple.directions(name: "Lalibela", address: "Naamsestraat 12, Leuven", lat: 50.88, lng: 4.7)?.absoluteString,
            "https://maps.apple.com/?daddr=50.88,4.7&q=Lalibela"
        )
        XCTAssertEqual(
            MapsApp.google.directions(name: "Lalibela", address: "Naamsestraat 12, Leuven", lat: 50.88, lng: 4.7)?.absoluteString,
            "comgooglemaps://?daddr=Lalibela,%20Naamsestraat%2012,%20Leuven&directionsmode=walking"
        )
    }

    func testGoogleMapsRoutesToTheCoordinatesWithoutAnAddress() {
        XCTAssertEqual(
            MapsApp.google.directions(name: "Lalibela", address: nil, lat: 50.88, lng: 4.7)?.absoluteString,
            "comgooglemaps://?daddr=50.88,4.7&directionsmode=walking"
        )
    }

    func testDirectionsEscapeNames() {
        XCTAssertEqual(
            MapsApp.apple.directions(name: "Fish & Chips #1 = Café", address: nil, lat: 50.88, lng: 4.7)?.absoluteString,
            "https://maps.apple.com/?daddr=50.88,4.7&q=Fish%20%26%20Chips%20%231%20%3D%20Caf%C3%A9"
        )
    }

    func testEditorLinksPutTheCoordinatesBeforeTheName() {
        XCTAssertEqual(
            EditorApp.comaps.link(name: "Fish & Chips", lat: 50.88, lng: 4.7)?.absoluteString,
            "cm://map?ll=50.88,4.7&n=Fish%20%26%20Chips"
        )
        XCTAssertEqual(
            EditorApp.organicMaps.link(name: "Lalibela", lat: 50.88, lng: 4.7)?.absoluteString,
            "om://map?ll=50.88,4.7&n=Lalibela"
        )
    }

    func testDirectionsFallBackToAppleMapsWithoutGoogleMaps() {
        for preference in DirectionsPreference.allCases {
            XCTAssertEqual(preference.app(googleInstalled: false), .apple)
        }
        XCTAssertNil(DirectionsPreference.ask.app(googleInstalled: true))
        XCTAssertEqual(DirectionsPreference.apple.app(googleInstalled: true), .apple)
        XCTAssertEqual(DirectionsPreference.google.app(googleInstalled: true), .google)
    }
}

final class VisitTests: XCTestCase {
    private func place(_ visits: [Visit]) -> Place {
        Place(id: "p", name: "P", lat: 0, lng: 0, address: nil, url: nil, cuisine: nil, hours: nil,
              countries: [], distanceM: 0, visits: visits)
    }

    func testVisitedTodayFollowsTheLocalDayOfTheLatestVisit() {
        var brussels = Calendar(identifier: .gregorian)
        brussels.timeZone = TimeZone(identifier: "Europe/Brussels")!
        var tokyo = brussels
        tokyo.timeZone = TimeZone(identifier: "Asia/Tokyo")!
        // 2026-11-17 23:30 in Brussels (22:30 UTC), then an older visit.
        let latest = place([Visit(id: 2, at: 1_794_954_600, verified: false), Visit(id: 1, at: 1_794_000_000, verified: true)])
        let sameEvening = Date(timeIntervalSince1970: 1_794_955_560)  // 23:46 in Brussels, 07:46 next day in Tokyo
        XCTAssertTrue(latest.visitedToday(now: sameEvening, calendar: brussels))
        XCTAssertTrue(latest.visitedToday(now: sameEvening, calendar: tokyo), "the visit was 07:30 there, the same day")
        let nextDay = Date(timeIntervalSince1970: 1_794_954_600 + 86_400)
        XCTAssertFalse(latest.visitedToday(now: nextDay, calendar: brussels))
        XCTAssertFalse(place([]).visitedToday(now: sameEvening, calendar: brussels))
    }
}

@MainActor
final class RecentAreasTests: XCTestCase {
    private func area(_ id: String) -> Area {
        Area(id: id, name: id, kind: .city, detail: nil, lat: 0, lng: 0)
    }

    func testRememberingMovesRepeatsToTheFrontAndKeepsSix() {
        let recent = ["a", "b", "c", "d", "e", "f"].map(area)
        XCTAssertEqual(AppModel.remembering(area("c"), in: recent).map(\.id), ["c", "a", "b", "d", "e", "f"])
        XCTAssertEqual(AppModel.remembering(area("g"), in: recent).map(\.id), ["g", "a", "b", "c", "d", "e"])
    }
}
