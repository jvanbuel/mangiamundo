import XCTest
@testable import Mangiamundo

final class ReminderTests: XCTestCase {
    private var calendar: Calendar = {
        var c = Calendar(identifier: .gregorian)
        c.timeZone = TimeZone(identifier: "Europe/Brussels")!
        return c
    }()

    private func date(_ y: Int, _ m: Int, _ d: Int, _ h: Int) -> Date {
        calendar.date(from: DateComponents(year: y, month: m, day: d, hour: h))!
    }

    private func celebration(_ id: String, _ starts: String) -> Celebration {
        let json = """
        {"id":"\(id)","title":"\(id)","description":"","countries":["US"],"starts":"\(starts)","ends":"\(starts)",
         "approximate":false,"nearby":[{"code":"US","places":3}],"reminder":{"title":"\(id) is today","body":"Celebrate."}}
        """
        return try! JSONDecoder().decode(Celebration.self, from: Data(json.utf8))
    }

    private func event(_ id: String, progress: Int = 0, nearby: Int = 4) -> EventProgress {
        let json = """
        {"id":"\(id)","title":"\(id)","description":"Eat at 3 places.","countries":["JP"],"starts":"2026-12-01",
         "ends":"2026-12-31","target":3,"progress":\(progress),"earned":\(progress >= 3),"status":"upcoming","places_nearby":\(nearby)}
        """
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return try! decoder.decode(EventProgress.self, from: Data(json.utf8))
    }

    func testSchedulesTenInTheMorningLocalTime() {
        let plan = ReminderPlan.plan(celebrations: [celebration("thanksgiving", "2026-11-26")], events: [], now: date(2026, 11, 16, 12), calendar: calendar)
        XCTAssertEqual(plan.count, 1)
        XCTAssertEqual(plan[0].date, date(2026, 11, 26, 10))
        XCTAssertEqual(plan[0].country, "US")
    }

    func testSkipsWhatAlreadyHappened() {
        let plan = ReminderPlan.plan(celebrations: [celebration("a", "2026-11-16")], events: [], now: date(2026, 11, 16, 12), calendar: calendar)
        XCTAssertTrue(plan.isEmpty)
    }

    func testEventsGetAStartAndALastDaysReminder() {
        let plan = ReminderPlan.plan(celebrations: [], events: [event("japan", progress: 1)], now: date(2026, 11, 16, 12), calendar: calendar)
        XCTAssertEqual(plan.map(\.key), ["event-start:japan", "event-last:japan"])
        XCTAssertEqual(plan[1].date, date(2026, 12, 29, 10))
        XCTAssertEqual(plan[1].body, "2 more places to earn the badge before it ends.")
    }

    func testLeavesOutEarnedEventsAndOnesWithNothingNearby() {
        let plan = ReminderPlan.plan(celebrations: [], events: [event("done", progress: 3), event("far", nearby: 0)], now: date(2026, 11, 16, 12), calendar: calendar)
        XCTAssertTrue(plan.isEmpty)
    }
}
