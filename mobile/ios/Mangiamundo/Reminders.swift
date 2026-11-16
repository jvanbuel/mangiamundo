import Foundation
import UserNotifications

/// One notification to schedule on the device.
struct PlannedReminder: Equatable {
    /// Stable key: scheduling the same reminder again replaces it.
    let key: String
    let date: Date
    let title: String
    let body: String
    /// Country to open on tap, if any.
    let country: String?
    /// Open the badges tab on tap.
    var badges = false
}

/// Turns the calendar into local notifications: one at 10:00 on the first day of each
/// celebration and event, and a last-days nudge for events the user has not finished.
/// Reminders whose moment has passed are left out. Mirrors ReminderPlan on Android.
enum ReminderPlan {
    static let hour = 10
    static let lastChanceDays = 3
    /// iOS keeps at most 64 pending notifications per app; stay well below.
    static let max = 20

    static func plan(celebrations: [Celebration], events: [EventProgress], now: Date, calendar: Calendar = .current) -> [PlannedReminder] {
        func at(_ day: String, minusDays: Int = 0) -> Date? {
            let parts = day.split(separator: "-").compactMap { Int($0) }
            guard parts.count == 3,
                  let date = calendar.date(from: DateComponents(year: parts[0], month: parts[1], day: parts[2], hour: hour))
            else { return nil }
            return calendar.date(byAdding: .day, value: -minusDays, to: date)
        }
        var out: [PlannedReminder] = []
        for c in celebrations {
            guard let date = at(c.starts) else { continue }
            out.append(PlannedReminder(key: "celebration:\(c.id):\(c.starts)", date: date, title: c.reminder.title, body: c.reminder.body, country: c.nearby.first?.code))
        }
        for e in events where !e.earned && e.placesNearby > 0 {
            let nearby = e.placesNearby == 1 ? "is 1 place" : "are \(e.placesNearby) places"
            if let date = at(e.starts) {
                out.append(PlannedReminder(key: "event-start:\(e.id)", date: date, title: "\(e.title) starts today", body: "\(e.description) There \(nearby) nearby.", country: nil, badges: true))
            }
            let left = e.target - e.progress
            if let date = at(e.ends, minusDays: lastChanceDays - 1) {
                out.append(PlannedReminder(
                    key: "event-last:\(e.id)", date: date, title: "Last days of \(e.title)",
                    body: "\(left == 1 ? "1 more place" : "\(left) more places") to earn the badge before it ends.",
                    country: nil, badges: true
                ))
            }
        }
        return Array(out.filter { $0.date > now }.sorted { $0.date < $1.date }.prefix(max))
    }
}

/// Schedules reminders with the system and reports taps back to the app.
@MainActor
final class ReminderCenter: NSObject, UNUserNotificationCenterDelegate {
    private let center = UNUserNotificationCenter.current()
    private static let prefix = "mangiamundo."
    /// Called with (country, badges) when the user taps a reminder.
    var onOpen: ((String?, Bool) -> Void)?

    override init() {
        super.init()
        center.delegate = self
    }

    /// Asks for permission when needed. True when notifications may be shown.
    func requestPermission() async -> Bool {
        let settings = await center.notificationSettings()
        switch settings.authorizationStatus {
        case .authorized, .provisional, .ephemeral: return true
        case .notDetermined: return (try? await center.requestAuthorization(options: [.alert, .sound, .badge])) ?? false
        default: return false
        }
    }

    func isAllowed() async -> Bool {
        let status = await center.notificationSettings().authorizationStatus
        return status == .authorized || status == .provisional || status == .ephemeral
    }

    /// Replaces everything scheduled before with `plan`.
    func schedule(_ plan: [PlannedReminder]) async {
        await cancelAll()
        for reminder in plan {
            let content = UNMutableNotificationContent()
            content.title = reminder.title
            content.body = reminder.body
            content.sound = .default
            var info: [String: Any] = ["badges": reminder.badges]
            if let country = reminder.country { info["country"] = country }
            content.userInfo = info
            let parts = Calendar.current.dateComponents([.year, .month, .day, .hour, .minute], from: reminder.date)
            let trigger = UNCalendarNotificationTrigger(dateMatching: parts, repeats: false)
            try? await center.add(UNNotificationRequest(identifier: Self.prefix + reminder.key, content: content, trigger: trigger))
        }
    }

    func cancelAll() async {
        let pending = await center.pendingNotificationRequests().map(\.identifier).filter { $0.hasPrefix(Self.prefix) }
        center.removePendingNotificationRequests(withIdentifiers: pending)
    }

    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse
    ) async {
        let info = response.notification.request.content.userInfo
        let country = info["country"] as? String
        let badges = info["badges"] as? Bool ?? false
        await MainActor.run { onOpen?(country, badges) }
    }

    /// Show reminders even while the app is open.
    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification
    ) async -> UNNotificationPresentationOptions {
        [.banner, .sound]
    }
}
