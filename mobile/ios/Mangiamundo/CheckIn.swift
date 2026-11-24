import SwiftUI

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
                ? ChunkyButtonStyle(face: Theme.discovered, content: Theme.onDiscovered)
                : ChunkyButtonStyle())
            .disabled(model.busyPlace != nil)
            .opacity(model.busyPlace == place.id ? 0.6 : 1)
        }
    }
}
