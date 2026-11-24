import MapKit
import SwiftUI

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
