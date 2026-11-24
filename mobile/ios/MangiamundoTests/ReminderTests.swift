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

    private var now: Date { date(2026, 11, 16, 12) }

    private func decode(_ data: Data) throws -> [Reminder] {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return try decoder.decode([Reminder].self, from: data)
    }

    private func reminder(_ key: String, _ day: String, hour: Int = 10, url: String = "/?tab=badges") -> Reminder {
        Reminder(key: key, localDate: day, hour: hour, title: "\(key) title", body: "\(key) body", url: url)
    }

    private func requests(_ reminders: [Reminder]) -> [PlannedReminder] {
        ReminderSchedule.requests(for: reminders, now: now, calendar: calendar)
    }

    func testDecodesWhatTheServerSends() throws {
        let json = """
        [{"key":"celebration:burns-night:2027-01-25","local_date":"2027-01-25","hour":10,
          "title":"Burns Night is today","body":"Haggis.","url":"/?country=GB","tag":"celebration-burns-night"}]
        """
        let expected = Reminder(
            key: "celebration:burns-night:2027-01-25", localDate: "2027-01-25", hour: 10,
            title: "Burns Night is today", body: "Haggis.", url: "/?country=GB"
        )
        XCTAssertEqual(try decode(Data(json.utf8)), [expected])
    }

    /// A real response, written by the backend's `writes_contract_fixtures` test.
    func testSchedulesTheContractFixture() throws {
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .appendingPathComponent("../../../contract/reminders.json")
        let reminders = try decode(Data(contentsOf: url))
        XCTAssertFalse(reminders.isEmpty)
        XCTAssertTrue(reminders.allSatisfy { !$0.key.isEmpty && !$0.title.isEmpty && !$0.body.isEmpty && $0.hour == 10 })
        XCTAssertEqual(reminders.first?.key, "quests:2026-12")
        XCTAssertTrue(ReminderTarget(url: reminders.first?.url).badges)
        // Every link is one the app can open.
        XCTAssertTrue(reminders.allSatisfy { ReminderTarget(url: $0.url) != ReminderTarget() })
        // The fixture is from mid November 2026: seen from then, all of it is still to come.
        let plan = requests(reminders)
        XCTAssertEqual(plan.map(\.key), reminders.map(\.key))
        XCTAssertEqual(plan.first?.date, date(2026, 12, 1, 10))
    }

    func testSchedulesAtTheServersHourInLocalTime() {
        let plan = requests([reminder("a", "2026-11-26", url: "/?country=US")])
        XCTAssertEqual(plan, [PlannedReminder(key: "a", date: date(2026, 11, 26, 10), title: "a title", body: "a body", url: "/?country=US")])
        var tokyo = Calendar(identifier: .gregorian)
        tokyo.timeZone = TimeZone(identifier: "Asia/Tokyo")!
        let evening = ReminderSchedule.requests(for: [reminder("b", "2026-11-26", hour: 18)], now: now, calendar: tokyo)
        XCTAssertEqual(evening.first?.date, tokyo.date(from: DateComponents(year: 2026, month: 11, day: 26, hour: 18)))
    }

    func testSkipsWhatAlreadyHappenedAndWhatItCannotRead() {
        // Today's 10:00 has passed at noon, and yesterday is long gone; 18:00 today is still ahead.
        let plan = requests([
            reminder("a", "2026-11-16"), reminder("b", "2026-11-15"), reminder("c", "2026-11-16", hour: 18),
            reminder("d", "soon"), reminder("e", "2026-11-20", hour: 24), reminder("f", "2026-13-01"),
        ])
        XCTAssertEqual(plan.map(\.key), ["c"])
    }

    func testKeepsTheSoonestFewInOrder() {
        func day(_ offset: Int) -> String {
            let parts = calendar.dateComponents([.year, .month, .day], from: calendar.date(byAdding: .day, value: offset, to: now)!)
            return String(format: "%04d-%02d-%02d", parts.year!, parts.month!, parts.day!)
        }
        let mornings: [Reminder] = (1...40).reversed().map { reminder("c\($0)", day($0)) }
        let later: [Reminder] = (1...40).map { reminder("e\($0)", day($0), hour: 11) }
        let plan = requests(mornings + later)
        XCTAssertEqual(plan.count, ReminderSchedule.max)
        XCTAssertLessThan(ReminderSchedule.max, 64)
        XCTAssertEqual(plan.prefix(3).map(\.key), ["c1", "e1", "c2"])
        XCTAssertEqual(plan.last?.key, "e24")
    }

    func testAKeyIsScheduledOnce() {
        let plan = requests([reminder("a", "2026-11-26"), reminder("a", "2026-11-27")])
        XCTAssertEqual(plan.map(\.date), [date(2026, 11, 26, 10)])
    }

    func testReadsWhatATapOpens() {
        XCTAssertEqual(ReminderTarget(url: "/?country=JP"), ReminderTarget(country: "JP"))
        XCTAssertEqual(ReminderTarget(url: "/?tab=badges"), ReminderTarget(badges: true))
        XCTAssertEqual(ReminderTarget(url: "/?tab=badges&country=JP#top"), ReminderTarget(country: "JP", badges: true))
        XCTAssertEqual(ReminderTarget(url: "/"), ReminderTarget())
        XCTAssertEqual(ReminderTarget(url: "/?tab=map&country="), ReminderTarget())
        XCTAssertEqual(ReminderTarget(url: nil), ReminderTarget())
    }
}
