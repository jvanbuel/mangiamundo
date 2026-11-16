import Foundation
import Observation

struct Toast: Identifiable, Equatable {
    let id = UUID()
    let text: String
}

/// All app state and the actions that change it.
@MainActor
@Observable
final class AppModel {
    static let radii = [5, 10, 25]

    private(set) var world: World?
    private(set) var catalog: Catalog?
    private(set) var location: SavedLocation?
    private(set) var radiusKm = 10
    private(set) var discover: Discover?
    private(set) var loading = false
    private(set) var error: String?

    /// Code of the country whose places are shown.
    private(set) var selected: String?
    private(set) var places: [Place] = []
    private(set) var placesLoading = false
    private(set) var busyPlace: String?

    private(set) var locating = false
    private(set) var locateError: String?

    private(set) var projection = ProjectionKind.globe
    var camera = Camera()
    /// Short announcements (badges, discoveries, errors), newest last.
    private(set) var toasts: [Toast] = []

    @ObservationIgnored private let api: APIClient
    @ObservationIgnored private let locationService = LocationService()
    @ObservationIgnored private let defaults = UserDefaults.standard
    /// The last real device position, used to verify check-ins while browsing another city.
    @ObservationIgnored private var gpsFix: (lat: Double, lng: Double, at: Date)?
    @ObservationIgnored private var refreshTask: Task<Void, Never>?
    @ObservationIgnored private var placesTask: Task<Void, Never>?
    @ObservationIgnored private var started = false

    init() {
        let base = Bundle.main.object(forInfoDictionaryKey: "APIBaseURL") as? String
        api = APIClient(baseURL: base ?? "http://localhost:8080")
        if defaults.object(forKey: "mangiamundo.radius") != nil {
            radiusKm = defaults.integer(forKey: "mangiamundo.radius")
        }
        if let raw = defaults.string(forKey: "mangiamundo.projection"), let kind = ProjectionKind(rawValue: raw) {
            projection = kind
        }
        if let data = defaults.data(forKey: "mangiamundo.location"),
           let saved = try? JSONDecoder().decode(SavedLocation.self, from: data) {
            location = saved
            camera.lng = saved.lng
            camera.lat = saved.lat
        }
    }

    func start() async {
        guard !started else { return }
        started = true
        world = World.bundled()
        do {
            catalog = try await api.catalog()
        } catch {
            self.error = error.localizedDescription
            return
        }
        if location != nil { refresh() }
        // Follow the device when there is no location yet or the last one came from GPS.
        if location == nil {
            await locate(quiet: false)
        } else if location?.fromGPS == true, locationService.isAuthorized {
            await locate(quiet: true)
        }
    }

    func country(_ code: String) -> Country? {
        catalog?.countries.first { $0.code == code }
    }

    func setProjection(_ kind: ProjectionKind) {
        projection = kind
        defaults.set(kind.rawValue, forKey: "mangiamundo.projection")
    }

    func setRadius(_ km: Int) {
        radiusKm = km
        defaults.set(km, forKey: "mangiamundo.radius")
        select(nil)
        refresh()
    }

    func choose(_ result: GeoResult) {
        let name = result.name.split(separator: ",").first.map(String.init)
        move(to: SavedLocation(lat: result.lat, lng: result.lng, name: name, fromGPS: false))
    }

    private func move(to target: SavedLocation) {
        location = target
        if let data = try? JSONEncoder().encode(target) {
            defaults.set(data, forKey: "mangiamundo.location")
        }
        select(nil)
        refresh()
    }

    /// Moves to the device position. With `quiet`, failures leave the current location alone.
    func locate(quiet: Bool = false) async {
        guard !locating else { return }
        locating = true
        locateError = nil
        defer { locating = false }
        do {
            let fix = try await locationService.current()
            let lat = fix.coordinate.latitude, lng = fix.coordinate.longitude
            gpsFix = (lat, lng, Date())
            if let previous = location, previous.fromGPS,
               abs(previous.lat - lat) <= 0.01, abs(previous.lng - lng) <= 0.01 {
                return
            }
            let name = try? await api.reverseGeocode(lat: lat, lng: lng)
            move(to: SavedLocation(lat: lat, lng: lng, name: name, fromGPS: true))
        } catch {
            if !quiet || location == nil {
                locateError = error.localizedDescription
            }
        }
    }

    func search(_ query: String) async throws -> [GeoResult] {
        try await api.geocode(query)
    }

    func refresh() {
        guard let here = location else { return }
        refreshTask?.cancel()
        refreshTask = Task {
            loading = true
            error = nil
            do {
                let result = try await api.discover(lat: here.lat, lng: here.lng, radiusKm: radiusKm)
                guard !Task.isCancelled else { return }
                discover = result
            } catch {
                guard !Task.isCancelled else { return }
                self.error = error.localizedDescription
                discover = nil
            }
            loading = false
        }
    }

    func select(_ code: String?) {
        selected = code
        places = []
        placesTask?.cancel()
        guard let code, let here = location else {
            placesLoading = false
            return
        }
        placesTask = Task {
            placesLoading = true
            do {
                let result = try await api.places(lat: here.lat, lng: here.lng, radiusKm: radiusKm, country: code)
                guard !Task.isCancelled else { return }
                places = result
            } catch {
                guard !Task.isCancelled else { return }
                toast("Could not load places: \(error.localizedDescription)")
            }
            placesLoading = false
        }
    }

    /// A tap on the map: open the country if it is available here, otherwise say so.
    func tapCountry(code: String, name: String) {
        guard let discover else { return }
        if discover.countries.contains(where: { $0.code == code }) {
            select(code)
        } else {
            toast("No places from \(name) within \(radiusKm) km.")
        }
    }

    func toggleVisit(_ place: Place) async {
        guard let before = discover, let here = location, busyPlace == nil else { return }
        busyPlace = place.id
        defer { busyPlace = nil }
        do {
            var updated = place
            if !place.visited {
                var position: (lat: Double, lng: Double)?
                if let fix = gpsFix, Date().timeIntervalSince(fix.at) < 600 {
                    position = (fix.lat, fix.lng)
                }
                let result = try await api.checkIn(placeID: place.id, here: position)
                updated.visitedAt = Int(Date().timeIntervalSince1970)
                updated.verified = result.verified
            } else {
                try await api.undoCheckIn(placeID: place.id)
                updated.visitedAt = nil
                updated.verified = false
            }
            if let index = places.firstIndex(where: { $0.id == place.id }) {
                places[index] = updated
            }
            let after = try await api.discover(lat: here.lat, lng: here.lng, radiusKm: radiusKm)
            discover = after
            if updated.visited { celebrate(before: before, after: after, place: updated) }
        } catch {
            toast("That did not work: \(error.localizedDescription)")
        }
    }

    /// Announces everything a check-in unlocked by comparing progress before and after.
    private func celebrate(before: Discover, after: Discover, place: Place) {
        for code in place.countries {
            guard let was = before.countries.first(where: { $0.code == code }),
                  let now = after.countries.first(where: { $0.code == code }),
                  let info = country(code) else { continue }
            if now.status == .completed, was.status != .completed {
                toast("\(info.flag) \(info.name) complete")
            } else if was.visited == 0 {
                toast("\(info.flag) \(info.name) tasted. \(now.places - now.visited) more to complete it.")
            }
        }
        for badge in after.badges where badge.earned {
            if !before.badges.contains(where: { $0.id == badge.id && $0.earned }) {
                toast("Badge earned: \(badge.title)")
            }
        }
        if after.profile.level > before.profile.level {
            toast("Level \(after.profile.level)")
        }
    }

    func toast(_ text: String) {
        let toast = Toast(text: text)
        toasts.append(toast)
        Task {
            try? await Task.sleep(for: .seconds(4))
            toasts.removeAll { $0.id == toast.id }
        }
    }
}
