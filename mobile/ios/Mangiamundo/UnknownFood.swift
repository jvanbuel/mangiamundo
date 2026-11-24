import SwiftUI

/// "3 restaurants nearby don't say what food they serve": a button under the countries that
/// opens the list, where the user can say which country's food each serves.
struct UnknownFood: View {
    @Environment(AppModel.self) private var model
    @State private var places: [UnknownPlace] = []
    @State private var open = false
    /// The place a suggestion just added, opened once the sheet is gone.
    @State private var added: Place?

    var body: some View {
        Group {
            if !places.isEmpty {
                Button { open = true } label: {
                    Label(places.count == 1
                        ? "1 restaurant doesn't say what food it serves"
                        : "\(places.count) restaurants don't say what food they serve",
                        systemImage: "questionmark.circle")
                }
                .buttonStyle(ChunkyButtonStyle(face: Theme.panelRaised, content: Theme.text, small: true))
                .frame(maxWidth: .infinity)
                .padding(.top, 8)
            }
        }
        .task(id: model.location?.area.id) { places = await model.unknownPlaces() }
        .sheet(isPresented: $open, onDismiss: {
            if let place = added { model.openPlace(place) }
            added = nil
        }) {
            UnknownFoodSheet(places: $places, onAdded: { added = $0; open = false })
                .overlay(alignment: .top) { ToastStack(toasts: model.toasts) }
                .presentationDetents([.medium, .large])
                .presentationBackground(Theme.panel)
        }
    }
}

struct UnknownFoodSheet: View {
    @Environment(AppModel.self) private var model
    @Binding var places: [UnknownPlace]
    let onAdded: (Place) -> Void
    /// The place whose answer is being saved. One at a time.
    @State private var busy: String?

    /// Only countries with a cuisine can be the food a place serves.
    private var countries: [Country] {
        (model.catalog?.countries ?? []).filter { $0.cuisine != nil }.sorted { $0.name < $1.name }
    }

    private func answer(_ place: UnknownPlace, country: String?) {
        busy = place.id
        Task {
            if let country {
                let added = await model.suggestCuisine(place, country: country)
                busy = nil
                if let added {
                    places.removeAll { $0.id == place.id }
                    onAdded(added)
                }
            } else {
                let done = await model.noCuisine(place)
                busy = nil
                if done { places.removeAll { $0.id == place.id } }
            }
        }
    }

    var body: some View {
        NavigationStack {
            List {
                Section {
                    ForEach(places) { place in
                        VStack(alignment: .leading, spacing: 8) {
                            Text(place.name).font(Theme.font(.body, .bold))
                            Text(([distance(place.distanceM)] + [place.address].compactMap { $0 }).joined(separator: " · "))
                                .font(Theme.font(.footnote))
                                .foregroundStyle(Theme.muted)
                            HStack(spacing: 8) {
                                Menu {
                                    ForEach(countries, id: \.code) { c in
                                        Button("\(c.flag) \(c.name)") { answer(place, country: c.code) }
                                    }
                                } label: {
                                    if busy == place.id {
                                        ProgressRing(lineWidth: 2.5).frame(width: 13, height: 13)
                                    } else {
                                        Label("Food from…", systemImage: "fork.knife")
                                    }
                                }
                                .accessibilityLabel("Food \(place.name) serves")
                                Button("Nothing specific") { answer(place, country: nil) }
                            }
                            .buttonStyle(ChunkyButtonStyle(face: Theme.panelRaised, content: Theme.text, small: true))
                            .disabled(busy != nil)
                        }
                        .padding(.vertical, 4)
                        .listRowBackground(Theme.panel)
                    }
                } footer: {
                    Text("A country you pick counts for you right away. We ask OpenStreetMap's mappers to add it with a note.")
                }
            }
            .scrollContentBackground(.hidden)
            .navigationTitle("What food do they serve?")
            .navigationBarTitleDisplayMode(.inline)
        }
    }
}
