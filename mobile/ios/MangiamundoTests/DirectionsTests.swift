import XCTest
@testable import Mangiamundo

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
