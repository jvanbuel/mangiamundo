import SwiftUI

struct ContentView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.scenePhase) private var scenePhase
    @State private var pickerOpen = false
    @State private var settingsOpen = false
    /// Whether "Directions with" is a choice to make. Checked again on return to the foreground,
    /// since Google Maps may have been installed or removed in the meantime.
    @State private var googleInstalled = false

    private var selected: CountryProgress? {
        model.discover?.countries.first { $0.code == model.selected }
    }

    var body: some View {
        @Bindable var model = model
        GeometryReader { geometry in
            VStack(spacing: 0) {
                ZStack {
                    Theme.bg
                    if let world = model.world {
                        WorldMapView(
                            world: world,
                            statuses: statuses,
                            selected: model.selected,
                            here: model.location,
                            kind: model.projection,
                            camera: $model.camera,
                            onTap: { model.tapCountry(code: $0.code, name: $0.name) }
                        )
                    }
                }
                .overlay(alignment: .topTrailing) { projectionPicker.padding(8) }
                .overlay(alignment: .bottomLeading) { Legend().padding(8) }
                .frame(height: geometry.size.height * 0.42)

                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 12) {
                        Header(onChange: { pickerOpen = true }, onSettings: { settingsOpen = true })
                        content
                    }
                    .padding(16)
                }
                .background(Theme.panel)
            }
        }
        .background(Theme.bg.ignoresSafeArea())
        .foregroundStyle(Theme.text)
        .font(Theme.font(.body))
        .overlay { if let progress = model.areaProgress { AreaProgressView(progress: progress) } }
        .overlay(alignment: .top) { if model.detail == nil { ToastStack(toasts: model.toasts) } }
        .onAppear { googleInstalled = MapsApp.googleInstalled }
        .onChange(of: scenePhase) { _, phase in
            if phase == .active { googleInstalled = MapsApp.googleInstalled }
        }
        .sheet(item: Binding(get: { model.detail }, set: { if $0 == nil { model.closePlace() } })) { place in
            ScrollView {
                PlaceDetail(place: place).padding(20)
            }
            // The sheet covers the screen's toasts, so it shows them itself.
            .overlay(alignment: .top) { ToastStack(toasts: model.toasts) }
            .presentationDetents([.medium, .large])
            .presentationBackground(Theme.panel)
        }
        .sheet(isPresented: $pickerOpen) {
            ScrollView {
                LocationPicker(onDone: { pickerOpen = false }).padding(20)
            }
            .presentationDetents([.medium, .large])
            .presentationBackground(Theme.panel)
        }
        .sheet(isPresented: $settingsOpen) {
            SettingsSheet(googleInstalled: googleInstalled)
                .padding(20)
                .presentationDetents([.medium])
                .presentationBackground(Theme.panel)
        }
    }

    private var statuses: [String: Status] {
        var result: [String: Status] = [:]
        for entry in model.discover?.countries ?? [] {
            result[entry.code] = entry.status
        }
        return result
    }

    private var projectionPicker: some View {
        Picker("Map projection", selection: Binding(get: { model.projection }, set: { model.setProjection($0) })) {
            Text("Globe").tag(ProjectionKind.globe)
            Text("Flat").tag(ProjectionKind.flat)
        }
        .pickerStyle(.segmented)
        .frame(width: 150)
    }

    @ViewBuilder
    private var content: some View {
        if model.location == nil {
            Card {
                VStack(alignment: .leading, spacing: 10) {
                    Text("Which of the world's kitchens are near you?")
                        .font(Theme.font(.title3, .black))
                    Text("Tell us where you are and we will light up every country whose kitchen you can visit nearby.")
                        .foregroundStyle(Theme.muted)
                    LocationPicker(onDone: {})
                }
            }
        } else if let error = model.error {
            Card {
                VStack(alignment: .leading, spacing: 10) {
                    Text(error)
                    Button("Try again") { model.refresh() }
                        .buttonStyle(.borderedProminent)
                }
            }
        } else if let discover = model.discover {
            if discover.world.available == 0 {
                Text("No restaurants with a national cuisine turned up in \(model.location?.area.name ?? "this area"). \(model.location?.area.kind == .postalCode ? "Try the whole city or another location." : "Try another location.")")
                    .foregroundStyle(Theme.muted)
            } else {
                Passport(discover: discover)
                if selected == nil && !model.celebrations.isEmpty {
                    ComingUp(celebrations: model.celebrations)
                }
                if let selected {
                    CountryDetail(progress: selected)
                } else {
                    Picker("Section", selection: Binding(get: { model.tab }, set: { model.tab = $0 })) {
                        Text("Countries \(discover.world.available)").tag(0)
                        Text("Badges \(discover.badges.filter(\.earned).count)/\(discover.badges.count)").tag(1)
                    }
                    .pickerStyle(.segmented)
                    if model.tab == 0 {
                        CountryList(discover: discover)
                    } else {
                        BadgeGrid(badges: discover.badges, events: discover.events)
                    }
                }
            }
        } else {
            Text("Scouting restaurants in \(model.location?.area.name ?? "your area")…")
                .foregroundStyle(Theme.muted)
        }
    }
}

/// Rarely needed things, kept off the main screen.
struct SettingsSheet: View {
    @Environment(AppModel.self) private var model
    @Environment(\.openURL) private var openURL
    let googleInstalled: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("Settings").font(Theme.font(.title3, .black))
            if googleInstalled { DirectionsSetting() }
            Button("Map data © OpenStreetMap contributors") {
                if let copyright = URL(string: "https://www.openstreetmap.org/copyright") { openURL(copyright) }
            }
            .buttonStyle(.plain)
            .font(Theme.font(.subheadline, .bold))
            .foregroundStyle(Theme.accent)
            .padding(.top, 16)
            BuildLine(server: model.server)
            Spacer()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// The "Directions with" setting. Only shown when Google Maps is installed.
struct DirectionsSetting: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        HStack {
            Text("Directions with").foregroundStyle(Theme.muted)
            Spacer()
            Picker("Directions with", selection: Binding(get: { model.directionsWith }, set: { model.setDirectionsWith($0) })) {
                ForEach(DirectionsPreference.allCases, id: \.self) { preference in
                    Text(preference.label).tag(preference)
                }
            }
            .pickerStyle(.menu)
            .tint(Theme.accent)
        }
        .font(Theme.font(.subheadline, .bold))
        .padding(.top, 16)
    }
}

/// Which builds of the app and the server are running, to tell old builds apart.
struct BuildLine: View {
    let server: String?

    var body: some View {
        let info = Bundle.main.infoDictionary ?? [:]
        let version = info["CFBundleShortVersionString"] as? String ?? "?"
        let build = info["CFBundleVersion"] as? String ?? "?"
        let commit = info["GitCommit"] as? String ?? "unknown"
        Text("App \(version) (\(build), \(commit))" + (server.map { " · Server \($0)" } ?? ""))
            .font(Theme.font(.caption2))
            .foregroundStyle(Theme.muted)
            .textSelection(.enabled)
            .padding(.top, 16)
    }
}
