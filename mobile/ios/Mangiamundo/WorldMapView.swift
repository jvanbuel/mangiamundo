import SwiftUI

/// The world as a globe or a flat map, drawn directly from the country outlines.
/// Drag to move, pinch to zoom, tap a country to pick it.
struct WorldMapView: View {
    let world: World
    let statuses: [String: Status]
    let selected: String?
    let here: SavedLocation?
    let kind: ProjectionKind
    @Binding var camera: Camera
    let onTap: (WorldCountry) -> Void

    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var dragStart: Camera?
    @State private var zoomStart: Double?
    @State private var glide: Task<Void, Never>?

    var body: some View {
        GeometryReader { geometry in
            let size = geometry.size
            // Redraws about 20 times a second only while there are diamonds to twinkle.
            let twinkle = !reduceMotion && statuses.values.contains(.completed)
            TimelineView(.animation(minimumInterval: 0.05, paused: !twinkle)) { timeline in
                Canvas { context, size in
                    draw(in: &context, size: size, time: twinkle ? timeline.date.timeIntervalSinceReferenceDate : nil)
                }
            }
            .contentShape(Rectangle())
            .gesture(
                DragGesture(minimumDistance: 4)
                    .onChanged { value in
                        glide?.cancel()
                        let start = dragStart ?? camera
                        dragStart = start
                        let perPoint = Projector(kind: kind, camera: camera, size: size).degreesPerPoint
                        // On the flat map a point covers fewer degrees of latitude towards the poles.
                        let latFactor = kind == .flat ? cos(start.lat * .pi / 180) : 1
                        var next = camera
                        next.lng = start.lng - Double(value.translation.width) * perPoint
                        next.lat = start.lat + Double(value.translation.height) * perPoint * latFactor
                        camera = next.normalized()
                    }
                    .onEnded { _ in dragStart = nil }
            )
            .simultaneousGesture(
                MagnifyGesture()
                    .onChanged { value in
                        let start = zoomStart ?? camera.zoom
                        zoomStart = start
                        var next = camera
                        next.zoom = start * Double(value.magnification)
                        camera = next.normalized()
                    }
                    .onEnded { _ in zoomStart = nil }
            )
            .onTapGesture { point in
                let projector = Projector(kind: kind, camera: camera, size: size)
                guard let at = projector.unproject(point),
                      let country = world.country(lng: at.lng, lat: at.lat) else { return }
                onTap(country)
            }
        }
        .clipped()
        .onChange(of: here) { _, target in
            guard let target else { return }
            glideTo(lng: target.lng, lat: target.lat)
        }
    }

    /// Glides to a position, taking the short way around the globe.
    private func glideTo(lng: Double, lat: Double) {
        glide?.cancel()
        let start = camera
        let delta = wrapLongitude(lng - start.lng)
        glide = Task {
            let steps = 40
            for step in 1...steps {
                try? await Task.sleep(for: .milliseconds(16))
                if Task.isCancelled { return }
                let t = Double(step) / Double(steps)
                let eased = t * t * (3 - 2 * t)
                var next = camera
                next.lng = start.lng + delta * eased
                next.lat = start.lat + (lat - start.lat) * eased
                camera = next.normalized()
            }
        }
    }
}
