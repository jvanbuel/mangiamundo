import XCTest
@testable import Mangiamundo

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
