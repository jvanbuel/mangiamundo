import SwiftUI

/// Rarely needed things, kept off the main screen.
struct SettingsSheet: View {
    @Environment(AppModel.self) private var model
    @Environment(\.openURL) private var openURL
    let googleInstalled: Bool

    var body: some View {
        ScrollView {
            content
        }
        .scrollBounceBehavior(.basedOnSize)
    }

    private var content: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("Settings").font(Theme.font(.title3, .black))
            if googleInstalled { DirectionsSetting() }
            AccountSection()
            Button("Map data © OpenStreetMap contributors") {
                if let copyright = URL(string: "https://www.openstreetmap.org/copyright") { openURL(copyright) }
            }
            .buttonStyle(.plain)
            .font(Theme.font(.subheadline, .bold))
            .foregroundStyle(Theme.accent)
            .padding(.top, 16)
            Button("Privacy policy") {
                if let privacy = URL(string: "https://mangiamundo.com/privacy/") { openURL(privacy) }
            }
            .buttonStyle(.plain)
            .font(Theme.font(.subheadline, .bold))
            .foregroundStyle(Theme.accent)
            .padding(.top, 12)
            BuildLine(server: model.server)
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
