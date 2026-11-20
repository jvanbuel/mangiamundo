import MapKit
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

/// A ring filling up while an area's restaurants load, with the step below it.
struct AreaProgressView: View {
    let progress: AreaProgress

    var body: some View {
        VStack(spacing: 10) {
            ZStack {
                Circle().stroke(Theme.line, lineWidth: 8)
                Circle()
                    .trim(from: 0, to: CGFloat(progress.percent) / 100)
                    .stroke(Theme.accent, style: StrokeStyle(lineWidth: 8, lineCap: .round))
                    .rotationEffect(.degrees(-90))
                Text("\(progress.percent)%").font(Theme.font(.title3, .extraBold))
            }
            .frame(width: 84, height: 84)
            .animation(.easeOut(duration: 0.4), value: progress.percent)
            Text(progress.text)
                .font(Theme.font(.subheadline, .bold))
                .foregroundStyle(Theme.muted)
        }
        .padding(20)
        .background(Theme.panel, in: RoundedRectangle(cornerRadius: 14))
        .padding(EdgeInsets(top: 2, leading: 2, bottom: 4, trailing: 2))
        .background(Theme.line, in: RoundedRectangle(cornerRadius: 16))
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(progress.text)
        .accessibilityValue("\(progress.percent) percent")
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
        case .pioneer: return "mappin.and.ellipse"
        case .unknown: return "star.fill"
        }
    }

    var color: Color {
        switch self {
        case .visits, .countries: return Theme.available
        case .completed, .world, .pioneer: return Theme.completed
        case .continent, .everything: return Theme.discovered
        case .unknown: return Theme.muted
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
    let onSettings: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Mangiamundo")
                        .font(Theme.font(.title2, .black))
                    Text("Eat the world, start with your city")
                        .font(Theme.font(.subheadline, .bold))
                        .foregroundStyle(Theme.muted)
                }
                Spacer()
                Button(action: onSettings) {
                    Image(systemName: "gearshape").font(Theme.font(.title3)).foregroundStyle(Theme.muted)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Settings")
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
            .task(id: query) { await suggest() }
            if let failure {
                Text(failure).font(Theme.font(.footnote)).foregroundStyle(Theme.danger)
            }

            if let results {
                if results.isEmpty {
                    Text("No city or postal code found. Try a different spelling.").font(Theme.font(.footnote)).foregroundStyle(Theme.muted)
                }
                ForEach(results) { result in
                    areaButton(result)
                }
            } else if query.trimmingCharacters(in: .whitespaces).isEmpty {
                let recent = model.recentAreas.filter { $0.id != model.location?.area.id }.prefix(5)
                if !recent.isEmpty {
                    Text("Recent")
                        .font(Theme.font(.subheadline, .extraBold))
                        .foregroundStyle(Theme.muted)
                        .accessibilityAddTraits(.isHeader)
                    ForEach(recent) { area in
                        HStack {
                            areaButton(area)
                            Button { model.forgetRecent(area.id) } label: {
                                Image(systemName: "xmark").font(.caption).foregroundStyle(Theme.muted).padding(8)
                            }
                            .buttonStyle(.plain)
                            .accessibilityLabel("Remove \(area.name) from recent")
                        }
                    }
                }
            }
        }
    }

    /// A city or postal code to move to, as in search results and recent areas.
    private func areaButton(_ area: Area) -> some View {
        Button {
            model.choose(area)
            onDone()
        } label: {
            VStack(alignment: .leading, spacing: 1) {
                Text(area.name)
                Text([area.kind == .city ? "City" : "Postal code", area.detail].compactMap { $0 }.joined(separator: " · "))
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

    /// Suggestions while typing. Failures are ignored: Search still works.
    private func suggest() async {
        let text = query.trimmingCharacters(in: .whitespaces)
        guard text.count >= 2 else {
            results = nil
            return
        }
        try? await Task.sleep(for: .milliseconds(250))
        guard !Task.isCancelled, let found = try? await model.suggest(text), !Task.isCancelled, !searching else { return }
        // An empty list while typing would show "not found" too early.
        if !found.isEmpty { results = found }
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

/// Directions and Website links for a place.
struct PlaceLinks: View {
    @Environment(AppModel.self) private var model
    @Environment(\.openURL) private var openURL
    let place: Place
    @State private var choosingMaps = false

    var body: some View {
        HStack(spacing: 16) {
            Button("Directions") {
                if let app = model.directionsWith.app(googleInstalled: MapsApp.googleInstalled) {
                    directions(app)
                } else {
                    choosingMaps = true
                }
            }
            .confirmationDialog("Directions to \(place.name)", isPresented: $choosingMaps, titleVisibility: .visible) {
                ForEach(MapsApp.allCases, id: \.self) { app in
                    Button(app.name) { directions(app) }
                }
            } message: {
                Text("To skip this question, pick an app under \"Directions with\" in Settings (the gear at the top).")
            }
            if let link = place.url, let url = URL(string: link) {
                Button("Website") { openURL(url) }
            }
        }
        .font(Theme.font(.subheadline, .extraBold))
        .buttonStyle(.plain)
        .foregroundStyle(Theme.accent)
    }

    private func directions(_ app: MapsApp) {
        if let url = app.directions(to: place) { openURL(url) }
    }
}

struct CheckInButton: View {
    @Environment(AppModel.self) private var model
    let place: Place

    var body: some View {
        // Redrawn every minute, so "Visited" turns into "Check in again" after midnight.
        TimelineView(.everyMinute) { context in
            let visited = place.visitedToday(now: context.date)
            Button {
                // The label may be up to a minute old: act only if it still says the same.
                guard place.visitedToday() == visited else { return }
                Task { await model.toggleVisit(place, visited: visited) }
            } label: {
                if visited {
                    Label("Visited", systemImage: "checkmark")
                } else {
                    Text(place.visited ? "Check in again" : "Check in")
                }
            }
            .buttonStyle(visited
                ? ChunkyButtonStyle(face: Theme.completed, content: Theme.onCompleted)
                : ChunkyButtonStyle())
            .disabled(model.busyPlace != nil)
            .opacity(model.busyPlace == place.id ? 0.6 : 1)
        }
    }
}

/// A place in a country's list. Tapping it (outside its links and button) opens the details.
struct PlaceRow: View {
    @Environment(AppModel.self) private var model
    let place: Place

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                VStack(alignment: .leading, spacing: 3) {
                    // A real button, so VoiceOver can open the details; the rest of the row is a tap area.
                    Button { model.openPlace(place) } label: {
                        Text(place.name).font(Theme.font(.headline, .extraBold)).multilineTextAlignment(.leading)
                    }
                    .buttonStyle(.plain)
                    .accessibilityHint("Shows details")
                    if let hours = place.hoursText {
                        Text(hours)
                            .font(Theme.font(.subheadline, .bold))
                            .foregroundStyle(place.openNow == true ? Theme.completed : Theme.muted)
                    }
                    Text(details)
                        .font(Theme.font(.subheadline))
                        .foregroundStyle(Theme.muted)
                    PlaceLinks(place: place).padding(.top, 2)
                }
                Spacer(minLength: 0)
                CheckInButton(place: place)
            }
            .padding(.vertical, 14)
            .contentShape(Rectangle())
            .onTapGesture { model.openPlace(place) }
            Divider().overlay(Theme.line)
        }
    }

    private var details: String {
        var parts = [distance(place.distanceM)]
        if let address = place.address { parts.append(address) }
        if place.verified { parts.append("checked in on the spot") }
        return parts.joined(separator: " · ")
    }
}

/// Everything about a place, with all of the user's visits. Shown in a sheet.
struct PlaceDetail: View {
    @Environment(AppModel.self) private var model
    @Environment(\.openURL) private var openURL
    let place: Place
    // Reports cannot be taken back, so the button only asks first.
    @State private var confirmingClosed = false

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(place.name)
                .font(Theme.font(.title2, .extraBold))
                .accessibilityAddTraits(.isHeader)
            if let cuisine = place.cuisine {
                // OSM cuisine tags read as words: "ethiopian;coffee_shop" becomes "ethiopian, coffee shop".
                Text(cuisine.replacingOccurrences(of: "_", with: " ").split(separator: ";").map { $0.trimmingCharacters(in: .whitespaces) }.joined(separator: ", "))
                    .font(Theme.font(.subheadline))
                    .foregroundStyle(Theme.muted)
            }
            Text(place.countries.map { code in model.country(code).map { "\($0.flag) \($0.name)" } ?? code }.joined(separator: "   "))
                .font(Theme.font(.subheadline, .bold))

            HStack(spacing: 16) {
                CheckInButton(place: place)
                PlaceLinks(place: place)
            }
            .padding(.vertical, 10)

            if !whereText.isEmpty {
                Text(whereText).font(Theme.font(.subheadline))
            }

            if let hours = place.hours {
                heading("Opening hours")
                if let status = hours.status {
                    Text(status.text)
                        .font(Theme.font(.subheadline, .bold))
                        .foregroundStyle(status.open ? Theme.completed : Theme.muted)
                }
                if let week = hours.week {
                    Grid(alignment: .leading, horizontalSpacing: 16, verticalSpacing: 4) {
                        ForEach(week, id: \.name) { day in
                            GridRow {
                                Text(day.name)
                                Text(day.open.isEmpty ? "Closed" : day.open.joined(separator: ", "))
                            }
                            .font(Theme.font(.subheadline, day.today ? .extraBold : .medium))
                        }
                    }
                } else {
                    Text(hours.raw).font(Theme.font(.subheadline)).foregroundStyle(Theme.muted)
                }
            }

            heading("Your visits")
            // Until the details load, the visits are the card's, which has only the latest one.
            switch model.detailVisits {
            case .loading:
                Text("Loading visits…").font(Theme.font(.subheadline)).foregroundStyle(Theme.muted)
            case .failed:
                Button("Could not load all visits. Try again") { model.openPlace(place) }
                    .font(Theme.font(.subheadline, .bold))
                    .foregroundStyle(Theme.accent)
            case .loaded:
                if place.visits.isEmpty {
                    Text("Not visited yet.").font(Theme.font(.subheadline)).foregroundStyle(Theme.muted)
                }
            }
            ForEach(place.visits) { visit in
                let date = Date(timeIntervalSince1970: TimeInterval(visit.at)).formatted(date: .complete, time: .omitted)
                Text(date + (visit.verified ? " · checked in on the spot" : ""))
                    .font(Theme.font(.subheadline))
                    .padding(.vertical, 6)
                Divider().overlay(Theme.line)
            }

            VStack(alignment: .leading, spacing: 4) {
                Button("Closed for good?") { confirmingClosed = true }
                    .disabled(model.busyPlace != nil)
                if let link = place.osmUrl, let url = URL(string: link) {
                    // A phone-friendly OSM editor when one is installed, otherwise the website.
                    let editor = EditorApp.installed
                    Button("Something wrong? Edit " + (editor.map { "in \($0.name)" } ?? "on OpenStreetMap")) {
                        // Read again when the user comes back from editing.
                        model.editingOnOsm(place.id)
                        // Checked again on tap: the app may have been uninstalled since the label was drawn.
                        openURL(EditorApp.installed?.link(name: place.name, lat: place.lat, lng: place.lng) ?? url)
                    }
                    Button(model.refreshingPlace == place.id ? "Refreshing from OpenStreetMap…" : "Refresh from OpenStreetMap") {
                        Task { await model.refreshPlace(place.id) }
                    }
                    .disabled(model.refreshingPlace != nil)
                    Button("Data © OpenStreetMap contributors") {
                        if let copyright = URL(string: "https://www.openstreetmap.org/copyright") { openURL(copyright) }
                    }
                }
            }
            .buttonStyle(.plain)
            .font(Theme.font(.caption))
            .foregroundStyle(Theme.muted)
            .padding(.top, 24)
            .confirmationDialog("Has \(place.name) closed for good?", isPresented: $confirmingClosed, titleVisibility: .visible) {
                Button("Yes, it closed", role: .destructive) { Task { await model.reportClosed(place) } }
            } message: {
                Text("It will be hidden for you, and for everyone once someone else reports it too.")
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var whereText: String {
        var parts: [String] = []
        if let address = place.address { parts.append(address) }
        if place.distanceM > 0 { parts.append("\(distance(place.distanceM)) away") }
        return parts.joined(separator: " · ")
    }

    private func heading(_ text: String) -> some View {
        Text(text)
            .font(Theme.font(.headline, .extraBold))
            .accessibilityAddTraits(.isHeader)
            .padding(.top, 14)
    }
}

func distance(_ meters: Int) -> String {
    meters < 1000 ? "\(meters / 10 * 10) m" : String(format: "%.1f km", Double(meters) / 1000)
}

/// "Add a missing restaurant": it goes on the map where the user stands.
struct AddPlaceForm: View {
    @Environment(AppModel.self) private var model
    @Environment(\.openURL) private var openURL
    let onCancel: () -> Void
    let onAdded: (Place) -> Void

    @State private var name = ""
    @State private var country = ""
    @State private var address = ""
    @State private var website = ""
    @State private var busy = false
    /// The map's centre, where the pin is; nil until the starting point is known.
    @State private var point: CLLocationCoordinate2D?
    @State private var camera: MapCameraPosition = .automatic

    /// A maps app the user can add the place to OSM with themselves.
    private let editor = EditorApp.installed

    /// Only countries with a cuisine can be the food a place serves.
    private var countries: [Country] {
        (model.catalog?.countries ?? []).filter { $0.cuisine != nil }.sorted { $0.name < $1.name }
    }

    private var ready: Bool { !busy && point != nil && trimmed(name) != nil && !country.isEmpty }

    /// Adds the place; with `note: false`, then opens the editor app at the pin.
    private func submit(note: Bool) {
        guard let point else { return }
        busy = true
        Task {
            let place = await model.addPlace(
                name: name.trimmingCharacters(in: .whitespacesAndNewlines),
                country: country,
                lat: point.latitude,
                lng: point.longitude,
                address: trimmed(address),
                website: trimmed(website),
                note: note
            )
            busy = false
            guard let place else { return }
            if !note, let url = editor?.link(name: place.name, lat: point.latitude, lng: point.longitude) { openURL(url) }
            onAdded(place)
        }
    }

    private func trimmed(_ text: String) -> String? {
        let value = text.trimmingCharacters(in: .whitespacesAndNewlines)
        return value.isEmpty ? nil : value
    }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField("Name", text: $name)
                    Picker("Food from", selection: $country) {
                        Text("Choose a country").tag("")
                        ForEach(countries, id: \.code) { Text("\($0.flag) \($0.name)").tag($0.code) }
                    }
                }
                Section {
                    if point != nil {
                        // Top-down only: with tilt, the region's centre is no longer under the pin.
                        Map(position: $camera, interactionModes: [.pan, .zoom]) { UserAnnotation() }
                            .onMapCameraChange(frequency: .continuous) { point = $0.region.center }
                            // The pin's tip marks the map's centre.
                            .overlay {
                                Image(systemName: "mappin")
                                    .font(.system(size: 34, weight: .bold))
                                    .foregroundStyle(Theme.accent)
                                    .offset(y: -17)
                                    .allowsHitTesting(false)
                                    .accessibilityHidden(true)
                            }
                            .frame(height: 220)
                            .listRowInsets(EdgeInsets())
                    } else {
                        Text("Finding your position…").foregroundStyle(Theme.muted)
                    }
                } header: {
                    Text("Where is it?")
                } footer: {
                    Text("Drag the map until the pin is on the restaurant's door.")
                }
                Section("Optional") {
                    TextField("Address", text: $address).textContentType(.fullStreetAddress)
                    TextField("Website", text: $website)
                        .keyboardType(.URL)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                }
                if let editor {
                    Section {
                        Button("Add it yourself in \(editor.name)") { submit(note: false) }
                            .disabled(!ready)
                    } footer: {
                        Text("Saves it here, then opens \(editor.name) at the pin: add it there with \"Add a place to the map\", and it's on OpenStreetMap right away. No note is sent.")
                    }
                }
                Section {
                    Button("Only you see it until it is on OpenStreetMap. We ask its mappers to add it by posting a note, shared under the Open Database License.") {
                        if let url = URL(string: "https://www.openstreetmap.org/copyright") { openURL(url) }
                    }
                    .buttonStyle(.plain)
                    .font(Theme.font(.caption))
                    .foregroundStyle(Theme.muted)
                }
            }
            .task {
                let start = await model.startingPoint()
                let centre = CLLocationCoordinate2D(latitude: start.lat, longitude: start.lng)
                camera = .camera(MapCamera(centerCoordinate: centre, distance: 300))
                point = centre
            }
            // Stays open until a submission finishes, so its result opens the right place.
            .interactiveDismissDisabled(busy)
            .navigationTitle("Add a restaurant")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel", action: onCancel).disabled(busy) }
                ToolbarItem(placement: .confirmationAction) {
                    Button(busy ? "Adding…" : "Add") { submit(note: true) }
                    .disabled(!ready)
                }
            }
        }
    }
}
