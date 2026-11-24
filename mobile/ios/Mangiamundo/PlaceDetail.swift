import SwiftUI

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

            if model.spun == place.id {
                HStack(spacing: 8) {
                    Button { Task { await model.spin() } } label: { Label("Spin again", systemImage: "dice.fill") }
                    Button("Not today") { Task { await model.notToday(place.id) } }
                }
                .buttonStyle(ChunkyButtonStyle(face: Theme.panelRaised, content: Theme.text, small: true))
                .disabled(model.spinning)
                .padding(.bottom, 10)
            }

            if !whereText.isEmpty {
                Text(whereText).font(Theme.font(.subheadline))
            }

            if let hours = place.hours {
                heading("Opening hours")
                if let status = hours.status {
                    Text(status.text)
                        .font(Theme.font(.subheadline, .bold))
                        .foregroundStyle(status.open ? Theme.text : Theme.muted)
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
                HStack {
                    Text(date + (visit.verified ? " · checked in on the spot" : ""))
                        .font(Theme.font(.subheadline))
                    Spacer()
                    Button { model.shareVisit(visit.id) } label: {
                        Label("Share", systemImage: "square.and.arrow.up")
                    }
                    .buttonStyle(ChunkyButtonStyle(small: true))
                    .disabled(model.sharing)
                }
                .padding(.vertical, 6)
                Divider().overlay(Theme.line)
            }

            VStack(alignment: .leading, spacing: 10) {
                HStack(spacing: 8) {
                    if let link = place.osmUrl, let url = URL(string: link) {
                        // A phone-friendly OSM editor when one is installed, otherwise the website.
                        let editor = EditorApp.installed
                        Button {
                            // Read again when the user comes back from editing.
                            model.editingOnOsm(place.id)
                            // Checked again on tap: the app may have been uninstalled since the label was drawn.
                            openURL(EditorApp.installed?.link(name: place.name, lat: place.lat, lng: place.lng) ?? url)
                        } label: {
                            Label("Edit", systemImage: "pencil")
                        }
                        .accessibilityLabel("Something wrong? Edit " + (editor.map { "in \($0.name)" } ?? "on OpenStreetMap"))
                        Button {
                            Task { await model.refreshPlace(place.id) }
                        } label: {
                            Label {
                                Text("Refresh")
                            } icon: {
                                if model.refreshingPlace == place.id {
                                    ProgressRing(lineWidth: 2.5).frame(width: 13, height: 13)
                                } else {
                                    Image(systemName: "arrow.clockwise")
                                }
                            }
                        }
                        .disabled(model.refreshingPlace != nil)
                        .accessibilityLabel(model.refreshingPlace == place.id ? "Refreshing from OpenStreetMap" : "Refresh from OpenStreetMap")
                    }
                    Button { confirmingClosed = true } label: {
                        Label("Closed?", systemImage: "xmark.circle")
                    }
                    .buttonStyle(ChunkyButtonStyle(face: Theme.panelRaised, content: Theme.danger, small: true))
                    .disabled(model.busyPlace != nil)
                    .accessibilityLabel("Closed for good?")
                }
                .buttonStyle(ChunkyButtonStyle(face: Theme.panelRaised, content: Theme.text, small: true))
                if place.osmUrl != nil {
                    Button("Data © OpenStreetMap contributors") {
                        if let copyright = URL(string: "https://www.openstreetmap.org/copyright") { openURL(copyright) }
                    }
                    .buttonStyle(.plain)
                    .font(Theme.font(.caption))
                    .foregroundStyle(Theme.muted)
                }
            }
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
