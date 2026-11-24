import SwiftUI

struct BadgeGrid: View {
    let badges: [Badge]
    var events: [EventProgress] = []
    var quests: MonthlyQuests?

    var body: some View {
        if let quests {
            ThisMonth(monthly: quests)
        }
        let running = events.filter { ($0.status == .upcoming || $0.status == .active) && !$0.earned }
        if !running.isEmpty {
            LimitedTime(events: running)
        }
        LazyVGrid(columns: Array(repeating: GridItem(.flexible(), alignment: .top), count: 3), spacing: 20) {
            ForEach(events.filter(\.earned)) { event in
                tile(Medal.event(), event.title, "\(shortDate(event.starts)) – \(shortDate(event.ends))", hint: event.description)
            }
            ForEach(badges.sorted { $0.earned && !$1.earned }) { badge in
                tile(
                    Medal(kind: badge.kind, earned: badge.earned),
                    badge.title,
                    badge.earned ? "Earned" : "\(badge.progress) / \(badge.target)",
                    hint: badge.description
                )
            }
        }
        .padding(.top, 8)
    }

    private func tile(_ medal: Medal, _ title: String, _ detail: String, hint: String) -> some View {
        VStack(spacing: 4) {
            medal
            Text(title)
                .font(Theme.font(.subheadline, .extraBold))
                .multilineTextAlignment(.center)
                .padding(.top, 4)
            Text(detail)
                .font(Theme.font(.footnote))
                .monospacedDigit()
                .foregroundStyle(Theme.muted)
        }
        .accessibilityElement(children: .combine)
        .accessibilityHint(hint)
    }
}

struct ThisMonth: View {
    let monthly: MonthlyQuests

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("This month").font(Theme.font(.headline, .black))
                Spacer()
                Text("Until \(shortDate(monthly.ends))").font(Theme.font(.caption, .bold)).foregroundStyle(Theme.muted)
            }
            ForEach(monthly.quests) { quest in
                HStack(alignment: .top, spacing: 12) {
                    Medal(symbol: "scope", face: .blue, earned: quest.done, size: 52)
                    VStack(alignment: .leading, spacing: 4) {
                        HStack {
                            Text(quest.title).font(Theme.font(.subheadline, .extraBold))
                            Spacer()
                            Text(quest.done ? "Done" : "+\(quest.xp) XP")
                                .font(Theme.font(.caption, .bold))
                                .foregroundStyle(Theme.muted)
                        }
                        Text(quest.description).font(Theme.font(.footnote)).foregroundStyle(Theme.muted)
                        HStack(spacing: 8) {
                            Bar(part: quest.progress, whole: quest.target)
                            Text("\(quest.progress) / \(quest.target)")
                                .font(Theme.font(.footnote))
                                .monospacedDigit()
                                .foregroundStyle(Theme.muted)
                        }
                    }
                }
                .accessibilityElement(children: .combine)
            }
        }
        .padding(.vertical, 8)
    }
}

struct LimitedTime: View {
    let events: [EventProgress]

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Limited time").font(Theme.font(.headline, .black))
            ForEach(events) { event in
                HStack(alignment: .top, spacing: 12) {
                    Medal.event(earned: event.status == .active, size: 52)
                    VStack(alignment: .leading, spacing: 4) {
                        HStack {
                            Text(event.title).font(Theme.font(.subheadline, .extraBold))
                            Spacer()
                            Text(timing(event))
                                .font(Theme.font(.caption, .extraBold))
                                .foregroundStyle(Theme.accentText)
                                .padding(.horizontal, 8)
                                .padding(.vertical, 2)
                                .background(Theme.event.opacity(0.16), in: Capsule())
                        }
                        Text(event.description).font(Theme.font(.footnote)).foregroundStyle(Theme.muted)
                        HStack(spacing: 8) {
                            Bar(part: event.progress, whole: event.target, color: Theme.event)
                            Text("\(event.progress) / \(event.target)")
                                .font(Theme.font(.footnote))
                                .monospacedDigit()
                                .foregroundStyle(Theme.muted)
                        }
                        if event.placesNearby == 0 {
                            Text("No places nearby for this one yet.").font(Theme.font(.footnote)).foregroundStyle(Theme.muted)
                        }
                    }
                }
            }
        }
        .padding(.vertical, 8)
    }

    private func timing(_ event: EventProgress) -> String {
        if event.status == .upcoming { return "Starts \(shortDate(event.starts))" }
        let left = daysFromToday(event.ends)
        return left <= 0 ? "Last day" : "\(left + 1) days left"
    }
}
