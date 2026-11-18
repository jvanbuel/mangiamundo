import XCTest
@testable import Mangiamundo

final class CoreTests: XCTestCase {
    private var world: World!

    override func setUpWithError() throws {
        world = try XCTUnwrap(World.bundled())
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
        // A location from before areas still reads as a legacy one.
        let old = try JSONDecoder().decode(LegacyLocation.self, from: Data(#"{"lat":50.88,"lng":4.7,"name":"Leuven","fromGPS":false}"#.utf8))
        XCTAssertEqual(old.lat, 50.88)
        XCTAssertNil(try? JSONDecoder().decode(SavedLocation.self, from: Data(#"{"lat":50.88,"lng":4.7,"name":"Leuven","fromGPS":false}"#.utf8)))
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
        let places = try decode([Place].self, "places")
        XCTAssertEqual(places.filter(\.visited).count, 2)
        XCTAssertEqual(places.filter(\.verified).count, 1)
        let celebrations = try decode([Celebration].self, "celebrations")
        XCTAssertTrue(celebrations.allSatisfy { !$0.nearby.isEmpty && !$0.reminder.title.isEmpty })
        XCTAssertTrue(try decode(VisitResult.self, "visit").verified)
        XCTAssertFalse(try decode([Area].self, "areas").isEmpty)
        XCTAssertEqual(try decode(Located.self, "located").options.count, 2)
    }

    func testToleratesWhatANewerServerSends() throws {
        let discover = try decode(Discover.self, "discover-newer")
        XCTAssertEqual(discover.badges[0].kind, .unknown)
        XCTAssertEqual(discover.events[0].status, .unknown)
    }
}

final class DirectionsTests: XCTestCase {
    func testDirectionsURLs() {
        XCTAssertEqual(MapsApp.apple.directions(lat: 50.88, lng: 4.7)?.absoluteString, "https://maps.apple.com/?daddr=50.88,4.7")
        XCTAssertEqual(
            MapsApp.google.directions(lat: 50.88, lng: 4.7)?.absoluteString,
            "comgooglemaps://?daddr=50.88,4.7&directionsmode=walking"
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
