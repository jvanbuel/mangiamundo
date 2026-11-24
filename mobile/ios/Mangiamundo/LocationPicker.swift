import SwiftUI

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
