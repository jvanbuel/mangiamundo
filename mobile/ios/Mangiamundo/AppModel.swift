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

    private(set) var celebrations: [Celebration] = []
    private(set) var remindersOn = false
    /// 0 = countries, 1 = badges.
    var tab = 0

    /// The recovery code once loaded for the backup sheet.
    private(set) var recoveryCode: String?

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
    @ObservationIgnored private let reminders = ReminderCenter()
    /// A country to open once progress has loaded, from a notification tap.
    @ObservationIgnored private var pendingCountry: String?

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
        reminders.onOpen = { [weak self] country, badges in self?.openFromNotification(country: country, badges: badges) }
        remindersOn = defaults.bool(forKey: "mangiamundo.reminders") ? await reminders.isAllowed() : false
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

    func dishes(of codes: [String]) -> [Dish] {
        catalog?.dishes(of: codes) ?? []
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
                if let code = pendingCountry {
                    pendingCountry = nil
                    if result.countries.contains(where: { $0.code == code }) { select(code) }
                }
                celebrations = (try? await api.celebrations(lat: here.lat, lng: here.lng, radiusKm: radiusKm)) ?? []
                await rescheduleReminders()
            } catch {
                guard !Task.isCancelled else { return }
                self.error = error.localizedDescription
                discover = nil
            }
            loading = false
        }
    }

    /// Opens what a notification points at.
    func openFromNotification(country: String?, badges: Bool) {
        if badges {
            select(nil)
            tab = 1
        }
        guard let country else { return }
        if discover?.countries.contains(where: { $0.code == country }) == true {
            select(country)
        } else {
            pendingCountry = country
        }
    }

    func toggleReminders() async {
        if remindersOn {
            remindersOn = false
            await reminders.cancelAll()
        } else if await reminders.requestPermission() {
            remindersOn = true
            await rescheduleReminders()
            toast("Reminders on. We will let you know when a country near you has something to celebrate.")
        } else {
            toast("Notifications are turned off for Mangiamundo in Settings.")
        }
        defaults.set(remindersOn, forKey: "mangiamundo.reminders")
    }

    private func rescheduleReminders() async {
        guard remindersOn else { return }
        await reminders.schedule(ReminderPlan.plan(celebrations: celebrations, events: discover?.events ?? [], now: Date()))
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

    private func replace(_ place: Place) {
        if let index = places.firstIndex(where: { $0.id == place.id }) {
            places[index] = place
        }
    }

    /// Runs a change to one place, then rereads progress and announces what it unlocked.
    private func change(_ place: Place, _ action: () async throws -> Place?) async {
        guard let before = discover, let here = location, busyPlace == nil else { return }
        busyPlace = place.id
        defer { busyPlace = nil }
        do {
            let updated = try await action()
            if let updated { replace(updated) }
            let after = try await api.discover(lat: here.lat, lng: here.lng, radiusKm: radiusKm)
            discover = after
            celebrate(before: before, after: after, place: updated)
        } catch {
            toast("That did not work: \(error.localizedDescription)")
        }
    }

    /// Checks in at a place: the first visit, or a return once the cooldown has passed.
    func checkIn(_ place: Place) async {
        await change(place) {
            var position: (lat: Double, lng: Double)?
            if let fix = gpsFix, Date().timeIntervalSince(fix.at) < 600 {
                position = (fix.lat, fix.lng)
            }
            let result = try await api.checkIn(placeID: place.id, here: position)
            let now = Int(Date().timeIntervalSince1970)
            var updated = place
            updated.visitedAt = place.visitedAt ?? now
            updated.lastVisitedAt = now
            updated.visits = place.visits + 1
            updated.verified = place.verified || result.verified
            updated.lastCheckinId = result.checkinId
            updated.lastDish = nil
            updated.nextCheckInAt = now + Rules.checkInCooldown
            if updated.visits > 1 {
                toast("Back at \(place.name): +\(Rules.xpPerVisit) XP. What did you have?")
            }
            return updated
        }
    }

    /// Logs the dish eaten at the place's latest check-in; picking it again clears it.
    func logDish(_ place: Place, _ dish: Dish) async {
        guard let checkin = place.lastCheckinId else { return }
        let chosen = place.lastDish == dish.id ? nil : dish.id
        let isNew = chosen != nil && discover?.dishesTried.contains(dish.id) == false
        await change(place) {
            _ = try await api.setDish(checkinID: checkin, dish: chosen)
            if isNew {
                toast("\(country(dish.country)?.flag ?? "") \(dish.name) tasted: +\(Rules.xpPerDish) XP")
            }
            var updated = place
            updated.lastDish = chosen
            return updated
        }
    }

    /// Takes back the place's latest check-in.
    func undoCheckIn(_ place: Place) async {
        guard let checkin = place.lastCheckinId, let here = location else { return }
        let code = selected
        await change(place) {
            try await api.undoCheckIn(checkinID: checkin)
            // The server knows what the previous check-in was; ask for the place again.
            guard let code else { return nil }
            let fresh = try await api.places(lat: here.lat, lng: here.lng, radiusKm: radiusKm, country: code)
            return fresh.first { $0.id == place.id }
        }
    }

    func loadRecoveryCode() async {
        guard recoveryCode == nil else { return }
        do {
            recoveryCode = try await api.recoveryCode()
        } catch {
            toast("Could not load your code: \(error.localizedDescription)")
        }
    }

    func rotateRecoveryCode() async {
        do {
            recoveryCode = try await api.rotateRecoveryCode()
        } catch {
            toast("Could not make a new code: \(error.localizedDescription)")
        }
    }

    /// Switches this device to the passport behind a recovery code.
    func restore(code: String) async -> Bool {
        do {
            let merged = try await api.restore(code: code)
            recoveryCode = nil
            select(nil)
            refresh()
            toast(merged > 0 ? "Passport restored. \(merged) check-ins from this phone came along." : "Passport restored. Welcome back.")
            return true
        } catch let error as APIError where error.status == 404 {
            toast("That code does not match a passport.")
        } catch {
            toast("Could not restore: \(error.localizedDescription)")
        }
        return false
    }

    /// The share card for the current place, as PNG data.
    func shareCard(dark: Bool) async -> Data? {
        guard let here = location else { return nil }
        do {
            return try await api.shareCard(lat: here.lat, lng: here.lng, radiusKm: radiusKm, place: here.name, dark: dark)
        } catch {
            toast("Could not draw the card: \(error.localizedDescription)")
            return nil
        }
    }

    /// Announces everything a check-in unlocked by comparing progress before and after.
    private func celebrate(before: Discover, after: Discover, place: Place?) {
        for code in place?.countries ?? [] {
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
        for event in after.events where event.earned {
            if !before.events.contains(where: { $0.id == event.id && $0.earned }) {
                toast("Badge earned: \(event.title)")
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
