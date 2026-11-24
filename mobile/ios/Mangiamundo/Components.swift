import SwiftUI

/// A panel with a solid outline and a heavier lower edge, rather than a soft tinted card.
struct Card<Content: View>: View {
    private let content: Content

    init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    var body: some View {
        content
            .padding(16)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Theme.panel, in: RoundedRectangle(cornerRadius: 14))
            .padding(EdgeInsets(top: 2, leading: 2, bottom: 4, trailing: 2))
            .background(Theme.line, in: RoundedRectangle(cornerRadius: 16))
    }
}

/// A solid, pressable button face on a darker lower edge; the face drops onto the edge while pressed.
struct ChunkyButtonStyle: ButtonStyle {
    var face = Theme.accent
    var content = Theme.onAccent
    /// A smaller button, for secondary actions.
    var small = false

    func makeBody(configuration: Configuration) -> some View {
        let pressed = configuration.isPressed
        let edge: CGFloat = small ? 2 : 3
        configuration.label
            .font(Theme.font(small ? .footnote : .subheadline, .extraBold))
            .foregroundStyle(content)
            .padding(.horizontal, small ? 10 : 14)
            .padding(.vertical, small ? 6 : 9)
            .background(face, in: RoundedRectangle(cornerRadius: small ? 9 : 12))
            .offset(y: pressed ? edge : 0)
            .padding(.bottom, edge)
            .background(alignment: .bottom) {
                RoundedRectangle(cornerRadius: small ? 9 : 12).fill(face).brightness(-0.18)
            }
    }
}

func parseDay(_ day: String) -> Date? {
    let parts = day.split(separator: "-").compactMap { Int($0) }
    guard parts.count == 3 else { return nil }
    return Calendar.current.date(from: DateComponents(year: parts[0], month: parts[1], day: parts[2]))
}

func shortDate(_ day: String) -> String {
    parseDay(day)?.formatted(.dateTime.day().month(.abbreviated)) ?? day
}

func daysFromToday(_ day: String) -> Int {
    guard let date = parseDay(day) else { return 0 }
    return Calendar.current.dateComponents([.day], from: Calendar.current.startOfDay(for: Date()), to: date).day ?? 0
}

struct Bar: View {
    var part: Int
    var whole: Int
    var color: AnyShapeStyle = AnyShapeStyle(Theme.accent)
    var height: CGFloat = 6

    init(part: Int, whole: Int, color: Color = Theme.accent, height: CGFloat = 6) {
        self.init(part: part, whole: whole, style: AnyShapeStyle(color), height: height)
    }

    init(part: Int, whole: Int, style: AnyShapeStyle, height: CGFloat = 6) {
        self.part = part
        self.whole = whole
        self.color = style
        self.height = height
    }

    var body: some View {
        GeometryReader { geometry in
            let fraction = whole == 0 ? 0 : min(max(CGFloat(part) / CGFloat(whole), 0), 1)
            ZStack(alignment: .leading) {
                // The empty part is outlined, so the bar shows even at zero.
                Capsule().fill(Theme.track).overlay(Capsule().stroke(Theme.trackEdge, lineWidth: 1))
                Capsule().fill(color).frame(width: geometry.size.width * fraction)
            }
        }
        .frame(height: height)
        .animation(.easeOut(duration: 0.4), value: part)
    }
}

struct Legend: View {
    var body: some View {
        HStack(spacing: 10) {
            ForEach(Status.allCases, id: \.self) { item($0) }
        }
        .font(Theme.font(.caption2, .semibold))
        .foregroundStyle(Theme.muted)
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(Theme.panel.opacity(0.92), in: RoundedRectangle(cornerRadius: 10))
        .overlay(RoundedRectangle(cornerRadius: 10).stroke(Theme.line))
    }

    private func item(_ status: Status) -> some View {
        HStack(spacing: 5) {
            StatusMark(status: status, size: 11)
            Text(status.label)
        }
    }
}

/// A country's state by shape as well as colour: a blue ring, a half-filled gold one,
/// a full platinum one, a diamond with a gem.
struct StatusMark: View {
    let status: Status
    var size: CGFloat = 14

    var body: some View {
        ZStack {
            switch status {
            case .available:
                Circle().strokeBorder(Theme.available, lineWidth: 2)
            case .discovered:
                Circle().fill(LinearGradient(stops: [.init(color: .clear, location: 0.5), .init(color: Theme.discovered, location: 0.5)], startPoint: .top, endPoint: .bottom))
                Circle().strokeBorder(Theme.discovered, lineWidth: 2)
            case .platinum:
                Circle().fill(Theme.platinumMetal)
                Circle().strokeBorder(Theme.platinumEdge, lineWidth: 2)
            case .completed:
                Circle().fill(Theme.diamond)
                Circle().strokeBorder(Theme.completedEdge, lineWidth: 2)
                Image(systemName: "diamond.fill")
                    .font(.system(size: size * 0.45, weight: .bold))
                    .foregroundStyle(Theme.onCompleted)
            }
        }
        .frame(width: size, height: size)
        .accessibilityHidden(true)
    }
}

func distance(_ meters: Int) -> String {
    meters < 1000 ? "\(meters / 10 * 10) m" : String(format: "%.1f km", Double(meters) / 1000)
}
