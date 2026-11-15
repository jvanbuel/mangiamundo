import SwiftUI

struct ContentView: View {
    @Environment(AppModel.self) private var model
    @State private var pickerOpen = false
    @State private var tab = 0

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
                        Header(onChange: { pickerOpen = true })
                        content
                    }
                    .padding(16)
                }
                .background(Theme.panel)
            }
        }
        .background(Theme.bg.ignoresSafeArea())
        .foregroundStyle(Theme.text)
        .overlay(alignment: .top) { ToastStack(toasts: model.toasts) }
        .sheet(isPresented: $pickerOpen) {
            ScrollView {
                LocationPicker(onDone: { pickerOpen = false }).padding(20)
            }
            .presentationDetents([.medium, .large])
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
                    Text("Eat your way around the world without leaving town.")
                        .font(Theme.display(22))
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
                Text("No restaurants with a national cuisine turned up within \(model.radiusKm) km. Try a larger radius or another location.")
                    .foregroundStyle(Theme.muted)
            } else {
                Passport(discover: discover)
                if let selected {
                    CountryDetail(progress: selected)
                } else {
                    Picker("Section", selection: $tab) {
                        Text("Countries \(discover.world.available)").tag(0)
                        Text("Badges \(discover.badges.filter(\.earned).count)/\(discover.badges.count)").tag(1)
                    }
                    .pickerStyle(.segmented)
                    if tab == 0 {
                        CountryList(discover: discover)
                    } else {
                        ForEach(discover.badges.sorted { $0.earned && !$1.earned }) { BadgeRow(badge: $0) }
                    }
                }
            }
        } else {
            Text("Scouting restaurants around \(model.location?.name ?? "you")…")
                .foregroundStyle(Theme.muted)
        }
    }
}
