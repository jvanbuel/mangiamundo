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
    // Everyone waiting gets the answer: startup and the add form can ask at the same time.
    private var authorizations: [CheckedContinuation<CLAuthorizationStatus, Never>] = []
    private var fixes: [CheckedContinuation<CLLocation, Error>] = []

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
                authorizations.append(continuation)
                manager.requestWhenInUseAuthorization()
            }
        }
        guard status == .authorizedWhenInUse || status == .authorizedAlways else {
            throw LocationError.denied
        }
        return try await withCheckedThrowingContinuation { continuation in
            fixes.append(continuation)
            manager.requestLocation()
        }
    }

    nonisolated func locationManagerDidChangeAuthorization(_ manager: CLLocationManager) {
        let status = manager.authorizationStatus
        Task { @MainActor in
            guard status != .notDetermined else { return }
            let waiting = authorizations
            authorizations = []
            waiting.forEach { $0.resume(returning: status) }
        }
    }

    nonisolated func locationManager(_ manager: CLLocationManager, didUpdateLocations locations: [CLLocation]) {
        guard let location = locations.last else { return }
        Task { @MainActor in
            let waiting = fixes
            fixes = []
            waiting.forEach { $0.resume(returning: location) }
        }
    }

    nonisolated func locationManager(_ manager: CLLocationManager, didFailWithError error: Error) {
        Task { @MainActor in
            let waiting = fixes
            fixes = []
            waiting.forEach { $0.resume(throwing: LocationError.unavailable) }
        }
    }
}
