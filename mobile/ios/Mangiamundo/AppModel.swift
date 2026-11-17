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

    private(set) var world: World?
    private(set) var catalog: Catalog?
    private(set) var location: SavedLocation?
    private(set) var discover: Discover?
    private(set) var loading = false
    private(set) var error: String?
    /// The first data is in (or failed), so the startup loader can go.
    private(set) var ready = false

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
        defaults.removeObject(forKey: "mangiamundo.radius")
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
            ready = true
            return
        }
        if location == nil { await moveLegacyLocation() }
        // Without a location the welcome screen shows right away; refresh() sets it otherwise.
        if location != nil { refresh() } else { ready = true }
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

    /// Picks a city or postal code from a search.
    func choose(_ area: Area) {
        move(to: SavedLocation(area: area, lat: area.lat, lng: area.lng, fromGPS: false))
    }

    /// Switches between the city and the postal code found at the GPS position.
    func switchArea(_ area: Area) {
        guard var target = location, target.area.id != area.id else { return }
        target.area = area
        move(to: target)
    }

    /// Moves a location saved before areas existed to the city at that point.
    private func moveLegacyLocation() async {
        guard let data = defaults.data(forKey: "mangiamundo.location"),
              let legacy = try? JSONDecoder().decode(LegacyLocation.self, from: data),
              let located = try? await api.locateArea(lat: legacy.lat, lng: legacy.lng),
              let area = located.city ?? located.postalCode,
              location == nil else { return } // a choice made meanwhile wins
        let target = SavedLocation(area: area, lat: legacy.lat, lng: legacy.lng, fromGPS: legacy.fromGPS, options: located.options)
        location = target
        if let data = try? JSONEncoder().encode(target) {
            defaults.set(data, forKey: "mangiamundo.location")
        }
    }

    /// Where the user is, when known recently: distances are measured from it.
    private var userPosition: (lat: Double, lng: Double)? {
        if let fix = gpsFix, Date().timeIntervalSince(fix.at) < 600 { return (fix.lat, fix.lng) }
        guard let location, location.fromGPS else { return nil }
        return (location.lat, location.lng)
    }

    private func move(to target: SavedLocation) {
        location = target
        // The old area's countries must not show while the new area loads.
        discover = nil
        celebrations = []
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
            // Background refreshes skip small moves; an explicit request always looks the area
            // up, since a short walk can cross a postal-code boundary.
            if quiet, let previous = location, previous.fromGPS,
               abs(previous.lat - lat) <= 0.01, abs(previous.lng - lng) <= 0.01 {
                return
            }
            let located = try await api.locateArea(lat: lat, lng: lng)
            // The city by default; someone who narrowed to their postal code before keeps that.
            let keepPostalCode = location?.fromGPS == true && location?.area.kind == .postalCode
            guard let area = (keepPostalCode ? located.postalCode : nil) ?? located.city ?? located.postalCode else {
                if !quiet || location == nil {
                    locateError = "No city or postal code found here. Search for your city instead."
                }
                return
            }
            move(to: SavedLocation(area: area, lat: lat, lng: lng, fromGPS: true, options: located.options))
        } catch {
            if !quiet || location == nil {
                locateError = error.localizedDescription
            }
        }
    }

    func search(_ query: String) async throws -> [Area] {
        try await api.searchAreas(query)
    }

    func refresh() {
        guard let here = location else { return }
        refreshTask?.cancel()
        refreshTask = Task {
            loading = true
            error = nil
            do {
                let result = try await api.discover(areaID: here.area.id)
                guard !Task.isCancelled else { return }
                discover = result
                ready = true
                if let code = pendingCountry {
                    pendingCountry = nil
                    if result.countries.contains(where: { $0.code == code }) { select(code) }
                }
                celebrations = (try? await api.celebrations(areaID: here.area.id)) ?? []
                await rescheduleReminders()
            } catch {
                guard !Task.isCancelled else { return }
                self.error = error.localizedDescription
                discover = nil
                ready = true
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
                let result = try await api.places(areaID: here.area.id, country: code, from: userPosition)
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
            toast("No places from \(name) in \(location?.area.name ?? "this area").")
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
            let after = try await api.discover(areaID: here.area.id)
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
