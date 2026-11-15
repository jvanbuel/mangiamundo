import CoreLocation

enum LocationError: LocalizedError {
    case denied, unavailable

    var errorDescription: String? {
        switch self {
        case .denied: return "Location access was denied. Search for your city instead."
        case .unavailable: return "Could not determine your location. Search for your city instead."
        }
    }
}

/// One-shot access to the device position, asking for permission when needed.
@MainActor
final class LocationService: NSObject, CLLocationManagerDelegate {
    private let manager = CLLocationManager()
    private var authorization: CheckedContinuation<CLAuthorizationStatus, Never>?
    private var fix: CheckedContinuation<CLLocation, Error>?

    override init() {
        super.init()
        manager.delegate = self
        manager.desiredAccuracy = kCLLocationAccuracyHundredMeters
    }

    /// True once the user has allowed location access.
    var isAuthorized: Bool {
        let status = manager.authorizationStatus
        return status == .authorizedWhenInUse || status == .authorizedAlways
    }

    func current() async throws -> CLLocation {
        var status = manager.authorizationStatus
        if status == .notDetermined {
            status = await withCheckedContinuation { continuation in
                authorization = continuation
                manager.requestWhenInUseAuthorization()
            }
        }
        guard status == .authorizedWhenInUse || status == .authorizedAlways else {
            throw LocationError.denied
        }
        return try await withCheckedThrowingContinuation { continuation in
            fix = continuation
            manager.requestLocation()
        }
    }

    nonisolated func locationManagerDidChangeAuthorization(_ manager: CLLocationManager) {
        let status = manager.authorizationStatus
        Task { @MainActor in
            guard status != .notDetermined else { return }
            authorization?.resume(returning: status)
            authorization = nil
        }
    }

    nonisolated func locationManager(_ manager: CLLocationManager, didUpdateLocations locations: [CLLocation]) {
        guard let location = locations.last else { return }
        Task { @MainActor in
            fix?.resume(returning: location)
            fix = nil
        }
    }

    nonisolated func locationManager(_ manager: CLLocationManager, didFailWithError error: Error) {
        Task { @MainActor in
            fix?.resume(throwing: LocationError.unavailable)
            fix = nil
        }
    }
}
