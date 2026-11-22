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

    /// Pixel offsets and sizes of a diamond's three sparkles at full size; the cluster is
    /// about 50 points across and shrinks to fit countries smaller than that on screen.
    private static let sparkleSpan: CGFloat = 50
    private static let sparkles: [(dx: CGFloat, dy: CGFloat, size: CGFloat)] = [(0, 0, 13), (17, -14, 8), (-19, 15, 6.5)]

    /// A four-pointed star: tips at `tip` from the centre, sides curving in to `waist`.
    private static func star(at c: CGPoint, tip: CGFloat, waist: CGFloat) -> Path {
        var path = Path()
        let dirs: [(CGFloat, CGFloat)] = [(0, -1), (1, 0), (0, 1), (-1, 0)]
        path.move(to: CGPoint(x: c.x, y: c.y - tip))
        for (i, (dx, dy)) in dirs.enumerated() {
            let (nx, ny) = dirs[(i + 1) % 4]
            path.addQuadCurve(
                to: CGPoint(x: c.x + nx * tip, y: c.y + ny * tip),
                control: CGPoint(x: c.x + (dx + nx) * waist, y: c.y + (dy + ny) * waist)
            )
        }
        path.closeSubpath()
        return path
    }

    private func draw(in context: inout GraphicsContext, size: CGSize, time: TimeInterval?) {
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
        var land = Path(), available = Path(), discovered = Path(), platinum = Path(), completed = Path()
        var borders = Path(), outline = Path()
        // Where each completed country's sparkles go: the middle of its largest visible ring,
        // scaled to that ring's size on screen so they stay within the country.
        var sparkleAt: [(point: CGPoint, scale: CGFloat)] = []
        // Bordering diamonds shine as one surface: one path per run of completed neighbours
        // (and per copy of the flat map).
        let regions = world.regions(Set(statuses.filter { $0.value == .completed }.keys))
        var gems: [String: Path] = [:]

        // The flat map repeats sideways, so draw the copies that are in view.
        let copies: [Double] = flat ? [-360, 0, 360] : [0]
        for country in world.countries {
            let status = statuses[country.code]
            var largest: (count: Int, sum: CGPoint, box: CGRect) = (0, .zero, .null)
            for shift in copies {
                var gem = Path()
                defer { if !gem.isEmpty, let region = regions[country.code] { gems["\(region) \(shift)", default: Path()].addPath(gem) } }
                if flat {
                    if projector.project(lng: country.maxLng + shift, lat: 0).point.x < 0 { continue }
                    if projector.project(lng: country.minLng + shift, lat: 0).point.x > size.width { continue }
                }
                for coordinates in country.rings where coordinates.count >= 6 {
                    var ring = Path()
                    var visible = false
                    var seen = 0, sum = CGPoint.zero, box = CGRect.null
                    for i in stride(from: 0, to: coordinates.count - 1, by: 2) {
                        let projected = projector.project(lng: coordinates[i] + shift, lat: coordinates[i + 1])
                        if projected.visible {
                            visible = true
                            seen += 1
                            sum.x += projected.point.x
                            sum.y += projected.point.y
                            box = box.union(CGRect(origin: projected.point, size: .zero))
                        }
                        if i == 0 { ring.move(to: projected.point) } else { ring.addLine(to: projected.point) }
                    }
                    if !visible { continue } // Entirely on the far side of the globe.
                    if status == .completed, seen > largest.count { largest = (seen, sum, box) }
                    ring.closeSubpath()
                    switch status {
                    case nil: land.addPath(ring)
                    case .available: available.addPath(ring)
                    case .discovered: discovered.addPath(ring)
                    case .platinum: platinum.addPath(ring)
                    case .completed: completed.addPath(ring); gem.addPath(ring)
                    }
                    borders.addPath(ring)
                    if country.code == selected { outline.addPath(ring) }
                }
            }
            if largest.count > 0 {
                let point = CGPoint(x: largest.sum.x / CGFloat(largest.count), y: largest.sum.y / CGFloat(largest.count))
                let fit = min(largest.box.width, largest.box.height) / Self.sparkleSpan
                sparkleAt.append((point, min(1, max(0.2, fit))))
            }
        }

        let evenOdd = FillStyle(eoFill: true)
        context.fill(land, with: .color(Theme.land), style: evenOdd)
        context.fill(available, with: .color(Theme.available), style: evenOdd)
        context.fill(discovered, with: .color(Theme.discovered), style: evenOdd)
        context.fill(platinum, with: .color(Theme.platinum), style: evenOdd)
        context.fill(completed, with: .color(Theme.completed), style: evenOdd)

        // Each diamond shines: lit from the top left, deeper towards the bottom right, and
        // now and then a glint of light sweeps across it (not with Reduce Motion).
        for gem in gems.values {
            let box = gem.boundingRect
            context.drawLayer { layer in
                layer.clip(to: gem, style: evenOdd)
                layer.fill(Path(box), with: .linearGradient(
                    Gradient(stops: [
                        .init(color: .white, location: 0),
                        .init(color: Theme.completed, location: 0.45),
                        .init(color: Theme.completedFacet, location: 1)
                    ]),
                    startPoint: CGPoint(x: box.minX, y: box.minY),
                    endPoint: CGPoint(x: box.maxX, y: box.maxY)
                ))
                guard let time else { return }
                // A 4-second cycle: the band crosses in the last 30% of it, then rests.
                let phase = (time / 4).truncatingRemainder(dividingBy: 1)
                guard phase > 0.7 else { return }
                let t = CGFloat((phase - 0.7) / 0.3)
                // A diagonal band, from the top-left corner to the bottom-right one: a white
                // heart with rainbow fringes, like light split by a cut stone.
                let c = CGPoint(x: box.minX + t * box.width, y: box.minY + t * box.height)
                let d = max(box.width, box.height) * 0.18
                layer.fill(Path(box), with: .linearGradient(
                    Gradient(stops: [
                        .init(color: .red.opacity(0), location: 0),
                        .init(color: .red.opacity(0.35), location: 0.15),
                        .init(color: .orange.opacity(0.45), location: 0.27),
                        .init(color: .yellow.opacity(0.55), location: 0.38),
                        .init(color: .white.opacity(0.9), location: 0.5),
                        .init(color: .cyan.opacity(0.55), location: 0.62),
                        .init(color: .blue.opacity(0.45), location: 0.73),
                        .init(color: .purple.opacity(0.35), location: 0.85),
                        .init(color: .purple.opacity(0), location: 1)
                    ]),
                    startPoint: CGPoint(x: c.x - d, y: c.y - d),
                    endPoint: CGPoint(x: c.x + d, y: c.y + d)
                ))
            }
        }

        // Thin borders everywhere; each state's countries get an outline in its own colour.
        // On the globe, rings that run over the horizon are flattened onto its rim, so the
        // outlines are clipped to just inside it rather than tracing the rim.
        context.drawLayer { layer in
            if !flat {
                let r = projector.radius - 1.5
                layer.clip(to: Path(ellipseIn: CGRect(x: centre.x - r, y: centre.y - r, width: 2 * r, height: 2 * r)))
            }
            layer.stroke(borders, with: .color(Theme.mapBorder), lineWidth: 0.6)
            layer.stroke(available, with: .color(Theme.availableEdge), lineWidth: 1)
            layer.stroke(discovered, with: .color(Theme.discoveredEdge), lineWidth: 1)
            layer.stroke(platinum, with: .color(Theme.platinumEdge), lineWidth: 1)
            layer.stroke(completed, with: .color(Theme.completedEdge), lineWidth: 2)
            layer.stroke(outline, with: .color(Theme.text), lineWidth: 2.5)
        }

        // Sparkles: the diamond's edge colour with a white heart. Each flashes on its own
        // beat, growing and brightening quickly and then shrinking back (full and still
        // with Reduce Motion).
        for (at, scale) in sparkleAt {
            for (i, sparkle) in Self.sparkles.enumerated() {
                let c = CGPoint(x: at.x + sparkle.dx * scale, y: at.y + sparkle.dy * scale)
                let wave = time.map { max(0, sin($0 * 2.6 + Double(i) * 2.1)) } ?? 1
                let flash = CGFloat(wave * wave)
                let tip = sparkle.size * scale * (0.55 + 0.45 * flash)
                var glint = context
                glint.opacity = 0.25 + 0.75 * flash
                glint.fill(Self.star(at: c, tip: tip, waist: tip * 0.25), with: .color(Theme.completedEdge))
                glint.fill(Self.star(at: c, tip: tip * 0.46, waist: tip * 0.11), with: .color(.white))
            }
        }

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
