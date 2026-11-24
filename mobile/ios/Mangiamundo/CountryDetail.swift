import SwiftUI

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
                part: min(progress.visited, progress.goal),
                whole: progress.goal,
                style: barStyle
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

    private var barStyle: AnyShapeStyle {
        switch progress.status {
        case .available: AnyShapeStyle(Theme.accent)
        case .discovered: AnyShapeStyle(Theme.discovered)
        case .platinum: AnyShapeStyle(Theme.platinumMetal)
        case .completed: AnyShapeStyle(Theme.diamondBar)
        }
    }

    private var summary: String {
        var text = "\(progress.visited) of \(progress.places) \(progress.places == 1 ? "place" : "places") visited"
        if progress.status != .available { text = "\(progress.status.label) · " + text }
        if progress.status != .completed { text += " · \(progress.goal - progress.visited) more for diamond" }
        return text
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
                            .foregroundStyle(place.openNow == true ? Theme.text : Theme.muted)
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
