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

    /// A route to the place. Only the coordinates: a name in the destination makes both apps search for it.
    func directions(lat: Double, lng: Double) -> URL? {
        switch self {
        case .apple:
            URL(string: "https://maps.apple.com/?daddr=\(lat),\(lng)")
        case .google:
            // The Google Maps app's URL scheme: no API key or SDK needed.
            URL(string: "comgooglemaps://?daddr=\(lat),\(lng)&directionsmode=walking")
        }
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
