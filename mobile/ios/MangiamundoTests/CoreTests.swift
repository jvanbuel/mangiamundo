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

    func testDecodesRepeatVisitsAndToleratesNewBadgeKinds() throws {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        let place = try decoder.decode(Place.self, from: Data("""
            {"id":"p","name":"Kaito","lat":50.8,"lng":4.7,"address":null,"url":null,"countries":["JP"],
             "distance_m":420,"visited_at":100,"verified":true,"visits":3,"last_visited_at":5000,
             "last_checkin_id":42,"last_dish":"jp-ramen","next_check_in_at":48200}
            """.utf8))
        XCTAssertEqual(place.visits, 3)
        XCTAssertEqual(place.lastCheckinId, 42)
        XCTAssertTrue(place.isRecent(at: Date(timeIntervalSince1970: 6000)))
        XCTAssertFalse(place.isRecent(at: Date(timeIntervalSince1970: 48200)))

        let badges = try decoder.decode([Badge].self, from: Data("""
            [{"id":"dish-1","kind":"dishes","title":"","description":"","progress":0,"target":1,"earned":false},
             {"id":"x","kind":"something-new","title":"","description":"","progress":0,"target":1,"earned":false}]
            """.utf8))
        XCTAssertEqual(badges.map(\.kind), [.dishes, .other])
    }
}
