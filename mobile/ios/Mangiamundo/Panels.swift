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

    func makeBody(configuration: Configuration) -> some View {
        let pressed = configuration.isPressed
        configuration.label
            .font(Theme.font(.subheadline, .extraBold))
            .foregroundStyle(content)
            .padding(.horizontal, 14)
            .padding(.vertical, 9)
            .background(face, in: RoundedRectangle(cornerRadius: 12))
            .offset(y: pressed ? 3 : 0)
            .padding(.bottom, 3)
            .background(alignment: .bottom) {
                RoundedRectangle(cornerRadius: 12).fill(face).brightness(-0.18)
            }
    }
}

extension BadgeKind {
    var symbol: String {
        switch self {
        case .visits: return "fork.knife"
        case .countries: return "flag.fill"
        case .completed: return "checkmark.seal.fill"
        case .continent: return "map.fill"
        case .world: return "globe.europe.africa.fill"
        case .everything: return "crown.fill"
        }
    }

    var color: Color {
        switch self {
        case .visits, .countries: return Theme.available
        case .completed, .world: return Theme.completed
        case .continent, .everything: return Theme.discovered
        }
    }
}

/// A solid token with a darker lower edge, so it reads as something you collect.
struct Medal: View {
    let symbol: String
    let color: Color
    var earned = true
    var size: CGFloat = 64

    init(kind: BadgeKind, earned: Bool = true, size: CGFloat = 64) {
        self.init(symbol: kind.symbol, color: kind.color, earned: earned, size: size)
    }

    init(symbol: String, color: Color, earned: Bool = true, size: CGFloat = 64) {
        self.symbol = symbol
        self.color = color
        self.earned = earned
        self.size = size
    }

    /// The medal of a limited-time event.
    static func event(earned: Bool = true, size: CGFloat = 64) -> Medal {
        Medal(symbol: "party.popper.fill", color: Theme.event, earned: earned, size: size)
    }

    var body: some View {
        let face = earned ? color : Theme.line
        ZStack {
            Circle().fill(face).brightness(-0.18)
            Circle().fill(face).padding(.bottom, size * 0.07)
            Image(systemName: symbol)
                .font(.system(size: size * 0.4, weight: .bold))
                .foregroundStyle(earned ? Color.white : Theme.muted)
                .offset(y: -size * 0.035)
        }
        .frame(width: size, height: size)
        .accessibilityHidden(true)
    }
}

private func parseDay(_ day: String) -> Date? {
    let parts = day.split(separator: "-").compactMap { Int($0) }
    guard parts.count == 3 else { return nil }
    return Calendar.current.date(from: DateComponents(year: parts[0], month: parts[1], day: parts[2]))
}

private func shortDate(_ day: String) -> String {
    parseDay(day)?.formatted(.dateTime.day().month(.abbreviated)) ?? day
}

private func daysFromToday(_ day: String) -> Int {
    guard let date = parseDay(day) else { return 0 }
    return Calendar.current.dateComponents([.day], from: Calendar.current.startOfDay(for: Date()), to: date).day ?? 0
}

struct BadgeGrid: View {
    let badges: [Badge]
    var events: [EventProgress] = []

    var body: some View {
        let running = events.filter { $0.status != .ended && !$0.earned }
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
                                .foregroundStyle(Theme.event)
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
                .foregroundStyle(.white)
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

struct Bar: View {
    var part: Int
    var whole: Int
    var color = Theme.accent
    var height: CGFloat = 6

    var body: some View {
        GeometryReader { geometry in
            let fraction = whole == 0 ? 0 : min(max(CGFloat(part) / CGFloat(whole), 0), 1)
            ZStack(alignment: .leading) {
                Capsule().fill(Theme.line)
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
            item(Theme.available, "To discover")
            item(Theme.discovered, "Tasted")
            item(Theme.completed, "Completed")
        }
        .font(Theme.font(.caption2, .semibold))
        .foregroundStyle(Theme.muted)
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(Theme.panel.opacity(0.92), in: RoundedRectangle(cornerRadius: 10))
        .overlay(RoundedRectangle(cornerRadius: 10).stroke(Theme.line))
    }

    private func item(_ color: Color, _ label: String) -> some View {
        HStack(spacing: 5) {
            RoundedRectangle(cornerRadius: 3).fill(color).frame(width: 9, height: 9)
            Text(label)
        }
    }
}

struct ToastStack: View {
    let toasts: [Toast]

    var body: some View {
        VStack(spacing: 8) {
            ForEach(toasts) { toast in
                Text(toast.text)
                    .font(Theme.font(.subheadline, .bold))
                    .padding(.horizontal, 14)
                    .padding(.vertical, 12)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(Theme.panelRaised, in: RoundedRectangle(cornerRadius: 14))
                    .overlay(RoundedRectangle(cornerRadius: 14).stroke(Theme.accent))
                    .shadow(color: .black.opacity(0.25), radius: 12, y: 6)
                    .transition(.move(edge: .top).combined(with: .opacity))
            }
        }
        .padding(.horizontal, 16)
        .padding(.top, 8)
        .animation(.spring(duration: 0.3), value: toasts)
        .allowsHitTesting(false)
    }
}

struct Header: View {
    @Environment(AppModel.self) private var model
    let onChange: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            VStack(alignment: .leading, spacing: 2) {
                Text("Mangiamundo")
                    .font(Theme.font(.title2, .black))
                Text("Eat the world, start with your city")
                    .font(Theme.font(.subheadline, .bold))
                    .foregroundStyle(Theme.muted)
            }
            if let location = model.location {
                Button(action: onChange) {
                    HStack(spacing: 6) {
                        Image(systemName: location.fromGPS ? "location.fill" : "mappin")
                            .font(Theme.font(.footnote))
                        Text(location.area.name)
                            .font(Theme.font(.body, .bold))
                            .lineLimit(1)
                        if location.area.kind == .postalCode {
                            Text("· postal code").foregroundStyle(Theme.muted)
                        }
                        Spacer()
                        Text("Change").font(Theme.font(.footnote)).foregroundStyle(Theme.accent)
                    }
                    .padding(.horizontal, 12)
                    .padding(.vertical, 10)
                    .overlay(RoundedRectangle(cornerRadius: 10).stroke(Theme.line))
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
        }
    }
}

struct LocationPicker: View {
    @Environment(AppModel.self) private var model
    let onDone: () -> Void

    @State private var query = ""
    @State private var results: [Area]?
    @State private var searching = false
    @State private var failure: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            // The city and postal code found at the GPS position.
            if let location = model.location, location.options.count > 1 {
                Picker("Area around you", selection: Binding(
                    get: { location.area.id },
                    set: { id in
                        if let option = location.options.first(where: { $0.id == id }) {
                            model.switchArea(option)
                            onDone()
                        }
                    }
                )) {
                    ForEach(location.options) { option in
                        Text(option.kind == .city ? "All of \(option.name)" : "Only \(option.name)").tag(option.id)
                    }
                }
                .pickerStyle(.segmented)
            }

            Button {
                Task {
                    await model.locate()
                    if model.locateError == nil { onDone() }
                }
            } label: {
                Label(model.locating ? "Finding you…" : "Use my current location", systemImage: "location.fill")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(ChunkyButtonStyle())
            .disabled(model.locating)

            if let error = model.locateError {
                Text(error).font(Theme.font(.footnote)).foregroundStyle(Theme.danger)
            }

            HStack {
                TextField("City or postal code", text: $query)
                    .textFieldStyle(.roundedBorder)
                    .submitLabel(.search)
                    .autocorrectionDisabled()
                    .onSubmit(search)
                Button(searching ? "…" : "Search", action: search)
                    .buttonStyle(.bordered)
                    .disabled(searching)
            }
            if let failure {
                Text(failure).font(Theme.font(.footnote)).foregroundStyle(Theme.danger)
            }

            if let results {
                if results.isEmpty {
                    Text("No city or postal code found. Try a different spelling.").font(Theme.font(.footnote)).foregroundStyle(Theme.muted)
                }
                ForEach(results) { result in
                    Button {
                        model.choose(result)
                        onDone()
                    } label: {
                        VStack(alignment: .leading, spacing: 1) {
                            Text(result.name)
                            Text([result.kind == .city ? "City" : "Postal code", result.detail].compactMap { $0 }.joined(separator: " · "))
                                .font(Theme.font(.caption))
                                .foregroundStyle(Theme.muted)
                        }
                        .multilineTextAlignment(.leading)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.vertical, 8)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
            }
        }
    }

    private func search() {
        let text = query.trimmingCharacters(in: .whitespaces)
        guard text.count >= 2, !searching else { return }
        searching = true
        failure = nil
        Task {
            do {
                results = try await model.search(text)
            } catch {
                failure = error.localizedDescription
            }
            searching = false
        }
    }
}

struct Passport: View {
    let discover: Discover

    var body: some View {
        let profile = discover.profile
        Card {
            VStack(alignment: .leading, spacing: 14) {
                VStack(spacing: 6) {
                    HStack(alignment: .firstTextBaseline) {
                        Text("Level \(profile.level)").font(Theme.font(.subheadline, .bold))
                        Spacer()
                        Text("\(profile.xp) / \(profile.nextLevelXp) XP")
                            .font(Theme.font(.subheadline))
                            .monospacedDigit()
                            .foregroundStyle(Theme.muted)
                    }
                    Bar(part: profile.xp - profile.levelXp, whole: profile.nextLevelXp - profile.levelXp)
                }

                VStack(spacing: 6) {
                    HStack(alignment: .firstTextBaseline) {
                        Text("Countries tasted").font(Theme.font(.subheadline, .bold))
                        Spacer()
                        (Text("\(discover.world.discovered)").font(Theme.font(.subheadline, .extraBold))
                            + Text(" of \(discover.world.available)").foregroundColor(Theme.muted))
                            .font(Theme.font(.subheadline))
                            .monospacedDigit()
                    }
                    Bar(part: discover.world.discovered, whole: discover.world.available)
                }

                Divider().overlay(Theme.line)
                HStack(alignment: .top) {
                    stat("Places visited", "\(discover.placesVisited) of \(discover.places)")
                    stat("Countries completed", "\(discover.world.completed)")
                }
            }
        }
    }

    private func stat(_ label: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label).font(Theme.font(.footnote)).foregroundStyle(Theme.muted)
            Text(value).font(Theme.font(.subheadline, .bold)).monospacedDigit()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

struct CountryList: View {
    @Environment(AppModel.self) private var model
    let discover: Discover

    var body: some View {
        ForEach(discover.continents.filter { $0.available > 0 }) { continent in
            VStack(alignment: .leading, spacing: 0) {
                HStack {
                    Text(continent.name).font(Theme.font(.headline, .extraBold))
                    Spacer()
                    Text("\(continent.discovered)/\(continent.available)")
                        .font(Theme.font(.subheadline))
                        .monospacedDigit()
                        .foregroundStyle(Theme.muted)
                }
                .padding(.horizontal, 8)
                .padding(.bottom, 4)

                ForEach(rows(in: continent)) { entry in
                    let info = model.country(entry.code)
                    Button {
                        model.select(entry.code)
                    } label: {
                        HStack(spacing: 10) {
                            Text(info?.flag ?? "").font(.title3).frame(width: 30)
                            Text(info?.name ?? entry.code)
                            Spacer()
                            Text("\(entry.visited)/\(entry.places)")
                                .font(Theme.font(.footnote))
                                .monospacedDigit()
                                .foregroundStyle(Theme.muted)
                            Circle().fill(Theme.color(for: entry.status)).frame(width: 10, height: 10)
                        }
                        .padding(8)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
            }
        }
    }

    private func rows(in continent: GroupProgress) -> [CountryProgress] {
        discover.countries
            .filter { $0.continent == continent.code }
            .sorted { (model.country($0.code)?.name ?? $0.code) < (model.country($1.code)?.name ?? $1.code) }
    }
}

struct CountryDetail: View {
    @Environment(AppModel.self) private var model
    let progress: CountryProgress

    var body: some View {
        let info = model.country(progress.code)
        VStack(alignment: .leading, spacing: 10) {
            Button { model.select(nil) } label: {
                Label("All countries", systemImage: "chevron.left")
            }
                .font(Theme.font(.subheadline, .bold))
                .foregroundStyle(Theme.muted)
                .buttonStyle(.plain)

            HStack(spacing: 14) {
                Text(info?.flag ?? "").font(.system(size: 44))
                VStack(alignment: .leading, spacing: 2) {
                    Text(info?.name ?? progress.code).font(Theme.font(.title2, .black))
                    Text(summary).font(Theme.font(.footnote)).foregroundStyle(Theme.muted)
                }
            }
            Bar(
                part: progress.visited,
                whole: progress.places,
                color: progress.status == .available ? Theme.accent : Theme.color(for: progress.status)
            )

            if model.placesLoading {
                Text("Loading places…").foregroundStyle(Theme.muted)
            }
            // The API sorts open places first, then closed, then unknown hours.
            let showGroups = model.places.contains { $0.openNow != nil }
            ForEach(Array(model.places.enumerated()), id: \.element.id) { index, place in
                if showGroups && (index == 0 || model.places[index - 1].openNow != place.openNow) {
                    Text(groupName(place.openNow))
                        .font(Theme.font(.footnote, .extraBold))
                        .textCase(.uppercase)
                        .foregroundStyle(Theme.muted)
                        .padding(.top, 14)
                }
                PlaceRow(place: place)
            }
        }
    }

    private func groupName(_ openNow: Bool?) -> String {
        switch openNow {
        case true?: "Open now"
        case false?: "Closed"
        case nil: "Hours unknown"
        }
    }

    private var summary: String {
        if progress.status == .completed {
            return "Completed · all \(progress.places) visited"
        }
        return "\(progress.visited) of \(progress.places) \(progress.places == 1 ? "place" : "places") visited"
    }
}

struct PlaceRow: View {
    @Environment(AppModel.self) private var model
    @Environment(\.openURL) private var openURL
    let place: Place

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                VStack(alignment: .leading, spacing: 3) {
                    Text(place.name).font(Theme.font(.headline, .extraBold))
                    if let hours = place.hoursText {
                        Text(hours)
                            .font(Theme.font(.subheadline, .bold))
                            .foregroundStyle(place.openNow == true ? Theme.completed : Theme.muted)
                    }
                    Text(details)
                        .font(Theme.font(.subheadline))
                        .foregroundStyle(Theme.muted)
                    HStack(spacing: 16) {
                        Button("Directions") {
                            if let url = URL(string: "https://maps.apple.com/?daddr=\(place.lat),\(place.lng)") {
                                openURL(url)
                            }
                        }
                        if let link = place.url, let url = URL(string: link) {
                            Button("Website") { openURL(url) }
                        }
                    }
                    .font(Theme.font(.subheadline, .extraBold))
                    .buttonStyle(.plain)
                    .foregroundStyle(Theme.accent)
                    .padding(.top, 2)
                }
                Spacer(minLength: 0)
                Button {
                    Task { await model.toggleVisit(place) }
                } label: {
                    if place.visited {
                        Label("Visited", systemImage: "checkmark")
                    } else {
                        Text("Check in")
                    }
                }
                .buttonStyle(place.visited
                    ? ChunkyButtonStyle(face: Theme.completed, content: Theme.onCompleted)
                    : ChunkyButtonStyle())
                .disabled(model.busyPlace != nil)
                .opacity(model.busyPlace == place.id ? 0.6 : 1)
            }
            .padding(.vertical, 14)
            Divider().overlay(Theme.line)
        }
    }

    private var details: String {
        let distance = place.distanceM < 1000
            ? "\(place.distanceM / 10 * 10) m"
            : String(format: "%.1f km", Double(place.distanceM) / 1000)
        var parts = [distance]
        if let address = place.address { parts.append(address) }
        if place.verified { parts.append("checked in on the spot") }
        return parts.joined(separator: " · ")
    }
}

