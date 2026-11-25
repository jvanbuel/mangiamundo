import XCTest
@testable import Mangiamundo

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
