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

    @State private var dragStart: Camera?
    @State private var zoomStart: Double?
    @State private var glide: Task<Void, Never>?

    var body: some View {
        GeometryReader { geometry in
            let size = geometry.size
            Canvas { context, size in
                draw(in: &context, size: size)
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

    private func draw(in context: inout GraphicsContext, size: CGSize) {
        let projector = Projector(kind: kind, camera: camera, size: size)
        let flat = kind == .flat
        let centre = CGPoint(x: size.width / 2, y: size.height / 2)

        if flat {
            context.fill(Path(CGRect(origin: .zero, size: size)), with: .color(Theme.ocean))
        } else {
            let r = projector.radius
            let disc = CGRect(x: centre.x - r, y: centre.y - r, width: 2 * r, height: 2 * r)
            context.fill(Path(ellipseIn: disc), with: .color(Theme.ocean))
        }

        // One path per fill colour plus one for all borders.
        var land = Path(), available = Path(), discovered = Path(), completed = Path()
        var borders = Path(), outline = Path()

        // The flat map repeats sideways, so draw the copies that are in view.
        let copies: [Double] = flat ? [-360, 0, 360] : [0]
        for country in world.countries {
            let status = statuses[country.code]
            for shift in copies {
                if flat {
                    if projector.project(lng: country.maxLng + shift, lat: 0).point.x < 0 { continue }
                    if projector.project(lng: country.minLng + shift, lat: 0).point.x > size.width { continue }
                }
                for coordinates in country.rings where coordinates.count >= 6 {
                    var ring = Path()
                    var visible = false
                    for i in stride(from: 0, to: coordinates.count - 1, by: 2) {
                        let projected = projector.project(lng: coordinates[i] + shift, lat: coordinates[i + 1])
                        if projected.visible { visible = true }
                        if i == 0 { ring.move(to: projected.point) } else { ring.addLine(to: projected.point) }
                    }
                    if !visible { continue } // Entirely on the far side of the globe.
                    ring.closeSubpath()
                    switch status {
                    case nil: land.addPath(ring)
                    case .available: available.addPath(ring)
                    case .discovered: discovered.addPath(ring)
                    case .completed: completed.addPath(ring)
                    }
                    borders.addPath(ring)
                    if country.code == selected { outline.addPath(ring) }
                }
            }
        }

        let evenOdd = FillStyle(eoFill: true)
        context.fill(land, with: .color(Theme.land), style: evenOdd)
        context.fill(available, with: .color(Theme.available), style: evenOdd)
        context.fill(discovered, with: .color(Theme.discovered), style: evenOdd)
        context.fill(completed, with: .color(Theme.completed), style: evenOdd)
        context.stroke(borders, with: .color(Theme.ocean), lineWidth: 0.6)
        context.stroke(outline, with: .color(Theme.text), lineWidth: 2)

        if let here {
            let marker = projector.project(lng: here.lng, lat: here.lat)
            if marker.visible {
                func dot(_ radius: CGFloat) -> Path {
                    Path(ellipseIn: CGRect(x: marker.point.x - radius, y: marker.point.y - radius, width: 2 * radius, height: 2 * radius))
                }
                context.fill(dot(12), with: .color(Theme.text.opacity(0.25)))
                context.fill(dot(6.5), with: .color(Theme.panel))
                context.fill(dot(4.5), with: .color(Theme.text))
            }
        }
    }
}
