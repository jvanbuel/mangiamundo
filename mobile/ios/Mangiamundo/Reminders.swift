import Foundation
import UserNotifications

/// One notification to schedule on the device.
struct PlannedReminder: Equatable {
    /// Stable key: scheduling the same reminder again replaces it.
    let key: String
    let date: Date
    let title: String
    let body: String
    /// What a tap opens; see `ReminderTarget`.
    let url: String
}

/// Turns the reminders the server plans (`/api/reminders`) into local notifications. Which
/// reminders there are and what they say is the server's business; the app only schedules.
/// Mirrors ReminderSchedule on Android.
enum ReminderSchedule {
    /// iOS keeps at most 64 pending notifications per app: only the soonest are scheduled.
    static let max = 48

    /// Each reminder at its hour on its day in the calendar's time zone, soonest first.
    /// Reminders whose moment has passed, or whose date can't be read, are left out.
    static func requests(for reminders: [Reminder], now: Date, calendar: Calendar = .current) -> [PlannedReminder] {
        var seen = Set<String>()
        var out: [PlannedReminder] = []
        for reminder in reminders {
            guard let date = moment(of: reminder, calendar: calendar), date > now, seen.insert(reminder.key).inserted else { continue }
            out.append(PlannedReminder(key: reminder.key, date: date, title: reminder.title, body: reminder.body, url: reminder.url))
        }
        // Reminders of the same moment stay in the server's order.
        let sorted = out.enumerated().sorted { ($0.element.date, $0.offset) < ($1.element.date, $1.offset) }
        return sorted.prefix(max).map(\.element)
    }

    private static func moment(of reminder: Reminder, calendar: Calendar) -> Date? {
        let parts = reminder.localDate.split(separator: "-").compactMap { Int($0) }
        guard parts.count == 3, (0..<24).contains(reminder.hour),
              let date = calendar.date(from: DateComponents(year: parts[0], month: parts[1], day: parts[2], hour: reminder.hour))
        else { return nil }
        // The calendar rolls a 13th month over into the next year; that is not a date.
        let read = calendar.dateComponents([.year, .month, .day], from: date)
        guard read.year == parts[0], read.month == parts[1], read.day == parts[2] else { return nil }
        return date
    }
}

/// What a tapped reminder opens: a country, the badges tab, or just the app.
struct ReminderTarget: Equatable {
    var country: String?
    var badges = false

    init(country: String? = nil, badges: Bool = false) {
        self.country = country
        self.badges = badges
    }

    /// Reads the server's link, such as `/?country=JP` or `/?tab=badges`.
    init(url: String?) {
        let items = url.flatMap { URLComponents(string: $0) }?.queryItems ?? []
        func value(_ name: String) -> String? { items.first { $0.name == name }?.value }
        if let code = value("country"), !code.isEmpty { country = code }
        badges = value("tab") == "badges"
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

    /// Replaces everything scheduled before with `plan`. A reminder scheduled before under
    /// the same key is replaced by the system, as the key is its identifier.
    func schedule(_ plan: [PlannedReminder]) async {
        let wanted = Set(plan.map { Self.prefix + $0.key })
        let stale = await center.pendingNotificationRequests().map(\.identifier)
            .filter { $0.hasPrefix(Self.prefix) && !wanted.contains($0) }
        center.removePendingNotificationRequests(withIdentifiers: stale)
        for reminder in plan {
            let content = UNMutableNotificationContent()
            content.title = reminder.title
            content.body = reminder.body
            content.sound = .default
            content.userInfo = ["url": reminder.url]
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
        let target = ReminderTarget(url: info["url"] as? String)
        await MainActor.run { onOpen?(target.country, target.badges) }
    }

    /// Show reminders even while the app is open.
    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification
    ) async -> UNNotificationPresentationOptions {
        [.banner, .sound]
    }
}
