import CoreGraphics
import Foundation

enum ProjectionKind: String {
    case globe, flat
}

/// What the map looks at: the centre of the view and a zoom factor (1 = whole world).
struct Camera: Equatable {
    var lng = 10.0
    var lat = 30.0
    var zoom = 1.0

    static let maxLat = 80.0
    static let minZoom = 0.8
    static let maxZoom = 8.0

    func normalized() -> Camera {
        Camera(
            lng: wrapLongitude(lng),
            lat: min(max(lat, -Camera.maxLat), Camera.maxLat),
            zoom: min(max(zoom, Camera.minZoom), Camera.maxZoom)
        )
    }
}

func wrapLongitude(_ lng: Double) -> Double {
    let turned = (lng + 180).truncatingRemainder(dividingBy: 360)
    return (turned < 0 ? turned + 360 : turned) - 180
}

private func radians(_ degrees: Double) -> Double { degrees * .pi / 180 }
private func degrees(_ radians: Double) -> Double { radians * 180 / .pi }

/// Maps between geographic coordinates (degrees) and view points. The globe is an
/// orthographic projection, the flat map a Web Mercator one.
struct Projector {
    let kind: ProjectionKind
    let size: CGSize
    /// Globe radius in points.
    let radius: Double
    /// Points per radian on the flat map; the world is `2π · scale` wide.
    let scale: Double

    private let lng0: Double
    private let lat0: Double
    private let sin0: Double
    private let cos0: Double
    private let mercator0: Double
    private static let maxMercator = radians(85)

    init(kind: ProjectionKind, camera: Camera, size: CGSize) {
        self.kind = kind
        self.size = size
        lng0 = radians(camera.lng)
        lat0 = radians(camera.lat)
        sin0 = sin(lat0)
        cos0 = cos(lat0)
        radius = Double(min(size.width, size.height)) / 2 * 0.92 * camera.zoom
        scale = Double(size.width) / (2 * .pi) * camera.zoom
        mercator0 = Projector.mercatorY(lat0)
    }

    /// Degrees of longitude (and, on the globe, latitude) that one point of dragging covers.
    var degreesPerPoint: Double {
        degrees(1 / (kind == .globe ? radius : scale))
    }

    /// The view position of a coordinate. `visible` is false when the point is on the far
    /// side of the globe; it is then placed on the edge of the disc, which keeps partly
    /// hidden outlines closed.
    func project(lng: Double, lat: Double) -> (point: CGPoint, visible: Bool) {
        let l = radians(lng) - lng0
        let p = radians(lat)
        let cx = Double(size.width) / 2, cy = Double(size.height) / 2
        if kind == .flat {
            return (CGPoint(x: cx + l * scale, y: cy - (Projector.mercatorY(p) - mercator0) * scale), true)
        }
        let cosP = cos(p)
        let x = cosP * sin(l)
        let y = cos0 * sin(p) - sin0 * cosP * cos(l)
        let facing = sin0 * sin(p) + cos0 * cosP * cos(l) >= 0
        let stretch = facing ? 1 : 1 / max(hypot(x, y), 1e-9)
        return (CGPoint(x: cx + x * stretch * radius, y: cy - y * stretch * radius), facing)
    }

    /// The coordinate under a view position, or nil outside the globe.
    func unproject(_ point: CGPoint) -> (lng: Double, lat: Double)? {
        let cx = Double(size.width) / 2, cy = Double(size.height) / 2
        if kind == .flat {
            let lng = degrees(lng0 + (Double(point.x) - cx) / scale)
            let lat = degrees(2 * atan(exp(mercator0 - (Double(point.y) - cy) / scale)) - .pi / 2)
            return (wrapLongitude(lng), lat)
        }
        let dx = (Double(point.x) - cx) / radius
        let dy = -(Double(point.y) - cy) / radius
        let rho = hypot(dx, dy)
        if rho > 1 { return nil }
        if rho < 1e-12 { return (wrapLongitude(degrees(lng0)), degrees(lat0)) }
        let c = asin(rho)
        let lat = asin(cos(c) * sin0 + dy * sin(c) * cos0 / rho)
        let lng = lng0 + atan2(dx * sin(c), rho * cos0 * cos(c) - dy * sin0 * sin(c))
        return (wrapLongitude(degrees(lng)), degrees(lat))
    }

    private static func mercatorY(_ lat: Double) -> Double {
        log(tan(.pi / 4 + min(max(lat, -maxMercator), maxMercator) / 2))
    }
}
