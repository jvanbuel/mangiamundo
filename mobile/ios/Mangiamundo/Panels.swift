import SwiftUI

struct Card<Content: View>: View {
    private let content: Content

    init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    var body: some View {
        content
            .padding(16)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Theme.panelRaised.opacity(0.55), in: RoundedRectangle(cornerRadius: 14))
            .overlay(RoundedRectangle(cornerRadius: 14).stroke(Theme.line))
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
            Text("Mangiamundo")
                .font(Theme.font(.title2, .black))
            if let location = model.location {
                Button(action: onChange) {
                    HStack(spacing: 6) {
                        Image(systemName: location.fromGPS ? "location.fill" : "mappin")
                            .font(Theme.font(.footnote))
                        Text(location.name ?? "Your location")
                            .font(Theme.font(.body, .bold))
                            .lineLimit(1)
                        Text("· \(model.radiusKm) km").foregroundStyle(Theme.muted)
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
    @State private var results: [GeoResult]?
    @State private var searching = false
    @State private var failure: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Button {
                Task {
                    await model.locate()
                    if model.locateError == nil { onDone() }
                }
            } label: {
                Label(model.locating ? "Finding you…" : "Use my current location", systemImage: "location.fill")
                    .font(Theme.font(.body, .extraBold))
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 10)
                    .foregroundStyle(Theme.onAccent)
                    .background(Theme.accent, in: RoundedRectangle(cornerRadius: 10))
            }
            .buttonStyle(.plain)
            .disabled(model.locating)

            if let error = model.locateError {
                Text(error).font(Theme.font(.footnote)).foregroundStyle(Theme.danger)
            }

            HStack {
                TextField("Search a city or address", text: $query)
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
                    Text("No matches. Try a different spelling.").font(Theme.font(.footnote)).foregroundStyle(Theme.muted)
                }
                ForEach(results) { result in
                    Button {
                        model.choose(result)
                        onDone()
                    } label: {
                        Text(result.name)
                            .multilineTextAlignment(.leading)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.vertical, 8)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
            }

            HStack {
                Text("Search radius").font(Theme.font(.footnote)).foregroundStyle(Theme.muted)
                Spacer()
                Picker("Search radius", selection: Binding(get: { model.radiusKm }, set: { model.setRadius($0) })) {
                    ForEach(AppModel.radii, id: \.self) { Text("\($0) km").tag($0) }
                }
                .pickerStyle(.segmented)
                .frame(width: 210)
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
                Divider().overlay(Theme.line)

                VStack(spacing: 8) {
                    ForEach(discover.continents) { continent in
                        let empty = continent.available == 0
                        HStack(spacing: 10) {
                            Text(continent.name).frame(width: 112, alignment: .leading)
                            Bar(part: continent.discovered, whole: continent.available)
                            Text(empty ? "–" : "\(continent.discovered)/\(continent.available)")
                                .monospacedDigit()
                                .foregroundStyle(Theme.muted)
                                .frame(width: 40, alignment: .trailing)
                        }
                        .font(Theme.font(.subheadline))
                        .foregroundStyle(empty ? Theme.muted : Theme.text)
                    }
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
            Button("← All countries") { model.select(nil) }
                .font(Theme.font(.footnote))
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
            ForEach(model.places) { place in
                PlaceRow(place: place)
            }
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
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 3) {
                Text(place.name).font(Theme.font(.body, .bold))
                Text(distance + (place.address.map { " · \($0)" } ?? ""))
                    .font(Theme.font(.footnote))
                    .foregroundStyle(Theme.muted)
                HStack(spacing: 14) {
                    Button("Directions") {
                        if let url = URL(string: "https://maps.apple.com/?daddr=\(place.lat),\(place.lng)") {
                            openURL(url)
                        }
                    }
                    if let link = place.url, let url = URL(string: link) {
                        Button("Website") { openURL(url) }
                    }
                    if place.verified {
                        Text("✓ verified").foregroundStyle(Theme.completed)
                    }
                }
                .font(Theme.font(.footnote))
                .buttonStyle(.plain)
                .foregroundStyle(Theme.accent)
            }
            Spacer(minLength: 0)
            Button {
                Task { await model.toggleVisit(place) }
            } label: {
                Text(place.visited ? "✓ Visited" : "Check in")
                    .font(Theme.font(.footnote, .extraBold))
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                    .foregroundStyle(place.visited ? Theme.onCompleted : Theme.accent)
                    .background(place.visited ? Theme.completed : Color.clear, in: Capsule())
                    .overlay(Capsule().stroke(place.visited ? Theme.completed : Theme.accent))
            }
            .buttonStyle(.plain)
            .disabled(model.busyPlace != nil)
        }
        .padding(12)
        .background(Theme.panelRaised, in: RoundedRectangle(cornerRadius: 12))
        .overlay(RoundedRectangle(cornerRadius: 12).stroke(place.visited ? Theme.completed.opacity(0.45) : .clear))
    }

    private var distance: String {
        place.distanceM < 1000
            ? "\(place.distanceM / 10 * 10) m"
            : String(format: "%.1f km", Double(place.distanceM) / 1000)
    }
}

struct BadgeRow: View {
    let badge: Badge

    var body: some View {
        HStack(spacing: 12) {
            Text(badge.icon)
                .font(.system(size: 26))
                .opacity(badge.earned ? 1 : 0.45)
                .grayscale(badge.earned ? 0 : 1)
            VStack(alignment: .leading, spacing: 2) {
                Text(badge.title).font(Theme.font(.body, .bold))
                Text(badge.description).font(Theme.font(.footnote)).foregroundStyle(Theme.muted)
                if !badge.earned {
                    HStack(spacing: 8) {
                        Bar(part: badge.progress, whole: badge.target)
                        Text("\(badge.progress)/\(badge.target)").font(Theme.font(.caption)).foregroundStyle(Theme.muted)
                    }
                    .padding(.top, 4)
                }
            }
            Spacer(minLength: 0)
            if badge.earned {
                Image(systemName: "checkmark").fontWeight(.bold).foregroundStyle(Theme.completed)
            }
        }
        .padding(12)
        .background(Theme.panelRaised, in: RoundedRectangle(cornerRadius: 12))
    }
}
