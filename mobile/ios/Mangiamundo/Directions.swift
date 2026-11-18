import UIKit

/// Which app the Directions link opens.
enum MapsApp: String, CaseIterable {
    case apple
    case google

    var name: String {
        switch self {
        case .apple: "Apple Maps"
        case .google: "Google Maps"
        }
    }

    /// A route to the place, carrying its name.
    ///
    /// Neither app has a parameter that labels a route's destination, so:
    /// - Apple Maps routes to the coordinates and gets the name as `q`, its label for a location.
    /// - Google Maps gets "name, address" as the destination and looks it up, so the route ends at
    ///   the place's own listing. Without an address that lookup could find a namesake elsewhere,
    ///   so it routes to the bare coordinates instead.
    func directions(to place: Place) -> URL? {
        directions(name: place.name, address: place.address, lat: place.lat, lng: place.lng)
    }

    func directions(name: String, address: String?, lat: Double, lng: Double) -> URL? {
        let coordinates = "\(lat),\(lng)"
        switch self {
        case .apple:
            return URL(string: "https://maps.apple.com/?daddr=\(coordinates)&q=\(Self.encode(name))")
        case .google:
            // The Google Maps app's URL scheme: no API key or SDK needed.
            let destination = address.map { "\(name), \($0)" } ?? coordinates
            return URL(string: "comgooglemaps://?daddr=\(Self.encode(destination))&directionsmode=walking")
        }
    }

    /// Percent-encodes a query value, including the `&`, `=`, `+` and `#` that URLComponents leaves alone.
    private static func encode(_ value: String) -> String {
        var allowed = CharacterSet.urlQueryAllowed
        allowed.remove(charactersIn: "&=+#?")
        return value.addingPercentEncoding(withAllowedCharacters: allowed) ?? value
    }

    /// Needs `comgooglemaps` in LSApplicationQueriesSchemes (project.yml).
    @MainActor
    static var googleInstalled: Bool {
        UIApplication.shared.canOpenURL(URL(string: "comgooglemaps://")!)
    }
}

/// The "Directions with" setting: ask each time, or always use one app.
enum DirectionsPreference: String, CaseIterable {
    case ask
    case apple
    case google

    var label: String {
        switch self {
        case .ask: "Ask"
        case .apple: MapsApp.apple.name
        case .google: MapsApp.google.name
        }
    }

    /// The app to open straight away, or nil to ask. Apple Maps whenever Google Maps isn't installed,
    /// including when it was chosen and later uninstalled.
    func app(googleInstalled: Bool) -> MapsApp? {
        guard googleInstalled else { return .apple }
        switch self {
        case .ask: return nil
        case .apple: return .apple
        case .google: return .google
        }
    }
}
