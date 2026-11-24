import SwiftUI

struct ComingUp: View {
    @Environment(AppModel.self) private var model
    let celebrations: [Celebration]

    var body: some View {
        Card {
            VStack(alignment: .leading, spacing: 0) {
                HStack {
                    Text("Coming up").font(Theme.font(.title3, .black))
                    Spacer()
                    Button {
                        Task { await model.toggleReminders() }
                    } label: {
                        Label(
                            model.remindersOn ? "Reminders on" : "Remind me",
                            systemImage: model.remindersOn ? "bell.badge.fill" : "bell"
                        )
                        .font(Theme.font(.subheadline, .extraBold))
                        .padding(.horizontal, 12)
                        .padding(.vertical, 6)
                        .overlay(Capsule().stroke(model.remindersOn ? .clear : Theme.line, lineWidth: 2))
                        .background(model.remindersOn ? Theme.accent.opacity(0.14) : .clear, in: Capsule())
                    }
                    .buttonStyle(.plain)
                    .foregroundStyle(Theme.accent)
                }
                .padding(.bottom, 6)

                ForEach(Array(celebrations.prefix(4).enumerated()), id: \.element.id) { index, c in
                    if index > 0 { Divider().overlay(Theme.line) }
                    Button {
                        if let code = c.nearby.first?.code { model.select(code) }
                    } label: {
                        HStack(spacing: 12) {
                            CalendarPage(day: c.starts)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(c.title).font(Theme.font(.headline, .extraBold))
                                Text(detail(c)).font(Theme.font(.footnote)).foregroundStyle(Theme.muted)
                            }
                            Spacer(minLength: 0)
                        }
                        .padding(.vertical, 10)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
            }
        }
    }

    private func detail(_ c: Celebration) -> String {
        let days = daysFromToday(c.starts)
        let when: String
        switch days {
        case ..<1: when = daysFromToday(c.ends) > 0 ? "On now" : "Today"
        case 1: when = "Tomorrow"
        default: when = "In \(days) days"
        }
        let total = c.nearby.reduce(0) { $0 + $1.places }
        let flags = c.nearby.compactMap { model.country($0.code)?.flag }.joined(separator: " ")
        return "\(when)\(c.approximate ? " (date may shift)" : "") · \(flags) \(total) \(total == 1 ? "place" : "places") nearby"
    }
}

/// A little tear-off calendar page.
struct CalendarPage: View {
    let day: String

    var body: some View {
        let date = parseDay(day) ?? Date()
        VStack(spacing: 0) {
            Text(date.formatted(.dateTime.month(.abbreviated)).uppercased())
                .font(Theme.font(.caption2, .extraBold))
                .foregroundStyle(Theme.onAccent)
                .frame(maxWidth: .infinity)
                .padding(.vertical, 2)
                .background(Theme.event)
            Text(date.formatted(.dateTime.day()))
                .font(Theme.font(.title3, .black))
                .padding(.vertical, 2)
        }
        .frame(width: 46)
        .background(Theme.panel)
        .clipShape(RoundedRectangle(cornerRadius: 8))
        .padding(EdgeInsets(top: 2, leading: 2, bottom: 3, trailing: 2))
        .background(Theme.line, in: RoundedRectangle(cornerRadius: 10))
    }
}
