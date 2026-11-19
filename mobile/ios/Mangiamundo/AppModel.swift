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
    /// Areas chosen before, newest first, to switch back to from the location picker.
    private(set) var recentAreas: [Area] = []
    private(set) var discover: Discover?
    private(set) var loading = false
    /// How far loading the area has come, while that takes long enough to show a wheel.
    private(set) var areaProgress: AreaProgress?
    private(set) var error: String?
    /// The first data is in (or failed), so the startup loader can go.
    private(set) var ready = false

    /// Code of the country whose places are shown.
    private(set) var selected: String?
    private(set) var places: [Place] = []
    private(set) var placesLoading = false
    private(set) var busyPlace: String?
    /// The place whose details are open: its card's data until the full place has loaded.
    private(set) var detail: Place?
    /// Whether `detail` holds all visits yet.
    private(set) var detailVisits = DetailVisits.loading
    @ObservationIgnored private var detailTask: Task<Void, Never>?

    private(set) var locating = false
    private(set) var locateError: String?

    private(set) var celebrations: [Celebration] = []
    private(set) var remindersOn = false
    /// 0 = countries, 1 = badges.
    var tab = 0
    /// The server's version and commit; nil until it answers, "not reached" after a failure.
    private(set) var server: String?

    private(set) var projection = ProjectionKind.globe
    private(set) var directionsWith = DirectionsPreference.ask
    var camera = Camera()
    /// Short announcements (badges, discoveries, errors), newest last.
    private(set) var toasts: [Toast] = []

    @ObservationIgnored private let api: APIClient
    @ObservationIgnored private let locationService = LocationService()
    @ObservationIgnored private let defaults = UserDefaults.standard
    /// The last real device position, used to verify check-ins while browsing another city.
    @ObservationIgnored private var gpsFix: (lat: Double, lng: Double, at: Date)?
    @ObservationIgnored private var refreshTask: Task<Void, Never>?
    /// Counts refreshes, so only the latest one shows or clears the progress wheel.
    @ObservationIgnored private var refreshes = 0
    @ObservationIgnored private var placesTask: Task<Void, Never>?
    @ObservationIgnored private var started = false
    @ObservationIgnored private let reminders = ReminderCenter()
    /// A country to open once progress has loaded, from a notification tap.
    @ObservationIgnored private var pendingCountry: String?

    init() {
        let base = Bundle.main.object(forInfoDictionaryKey: "APIBaseURL") as? String
        api = APIClient(baseURL: base ?? "http://localhost:8080")
        defaults.removeObject(forKey: "mangiamundo.radius")
        if let raw = defaults.string(forKey: "mangiamundo.directions"), let preference = DirectionsPreference(rawValue: raw) {
            directionsWith = preference
        }
        if let raw = defaults.string(forKey: "mangiamundo.projection"), let kind = ProjectionKind(rawValue: raw) {
            projection = kind
        }
        if let data = defaults.data(forKey: Self.recentKey),
           let saved = try? JSONDecoder().decode([Area].self, from: data) {
            recentAreas = saved
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
        loadServer()
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

    func setDirectionsWith(_ preference: DirectionsPreference) {
        directionsWith = preference
        defaults.set(preference.rawValue, forKey: "mangiamundo.directions")
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

    private static let recentKey = "mangiamundo.recentAreas"

    /// `area` at the front of `recent`, without duplicates, keeping at most 6.
    static func remembering(_ area: Area, in recent: [Area]) -> [Area] {
        Array(([area] + recent.filter { $0.id != area.id }).prefix(6))
    }

    func forgetRecent(_ id: String) {
        recentAreas.removeAll { $0.id == id }
        saveRecent()
    }

    private func saveRecent() {
        if let data = try? JSONEncoder().encode(recentAreas) {
            defaults.set(data, forKey: Self.recentKey)
        }
    }

    private func move(to target: SavedLocation) {
        // The area being left counts too, so a location saved before recents existed is kept.
        let left = location.map { Self.remembering($0.area, in: recentAreas) } ?? recentAreas
        location = target
        recentAreas = Self.remembering(target.area, in: left)
        saveRecent()
        // The old area's countries must not show while the new area loads.
        discover = nil
        celebrations = []
        if let data = try? JSONEncoder().encode(target) {
            defaults.set(data, forKey: "mangiamundo.location")
        }
        closePlace()
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

    func suggest(_ query: String) async throws -> [Area] {
        let near = gpsFix.map { ($0.lat, $0.lng) } ?? location.map { ($0.lat, $0.lng) }
        return try await api.suggestAreas(query, near: near)
    }

    /// Asks for the server's version until it answers once.
    private func loadServer() {
        guard server == nil || server == "not reached" else { return }
        Task {
            if let health = try? await api.health() {
                server = [health.version ?? "unknown", health.commit].compactMap { $0 }.joined(separator: " ")
            } else {
                server = "not reached"
            }
        }
    }

    func refresh() {
        loadServer()
        guard let here = location else { return }
        refreshTask?.cancel()
        refreshes += 1
        let refresh = refreshes
        refreshTask = Task {
            loading = true
            error = nil
            // A wheel from the previous area must not linger while this one starts.
            areaProgress = nil
            // A new area can take a while to load: show the server's progress after 0.3 s.
            let poll = Task {
                try? await Task.sleep(for: .milliseconds(300))
                while !Task.isCancelled {
                    // nil (not started or already done) and errors keep the last value. Only the
                    // latest refresh shows its wheel.
                    if let progress = try? await api.areaProgress(areaID: here.area.id), !Task.isCancelled, refresh == refreshes {
                        areaProgress = progress
                        // At startup the loader waits for the data: let the wheel show instead.
                        ready = true
                    }
                    try? await Task.sleep(for: .milliseconds(400))
                }
            }
            let fetched: Result<Discover, Error>
            do {
                fetched = .success(try await api.discover(areaID: here.area.id))
            } catch {
                fetched = .failure(error)
            }
            // The wheel is about the restaurants: it goes before celebrations and reminders load.
            poll.cancel()
            if refresh == refreshes { areaProgress = nil }
            do {
                let result = try fetched.get()
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

    /// Opens a place's details, with all of the user's visits.
    func openPlace(_ place: Place) {
        detail = place
        detailVisits = .loading
        detailTask?.cancel()
        // Measured from the user when known, otherwise from the map pin.
        let from = userPosition ?? location.map { (lat: $0.lat, lng: $0.lng) }
        detailTask = Task {
            do {
                let full = try await api.place(id: place.id, from: from)
                guard !Task.isCancelled else { return }
                detail = full
                detailVisits = .loaded
            } catch {
                guard !Task.isCancelled else { return }
                detailVisits = .failed
            }
        }
    }

    func closePlace() {
        detailTask?.cancel()
        detail = nil
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

    /// Checks in, or undoes today's visit. `visited` is what the button showed, so the
    /// action matches its label even if midnight passes in between.
    func toggleVisit(_ place: Place, visited: Bool? = nil) async {
        guard let before = discover, let here = location, busyPlace == nil else { return }
        busyPlace = place.id
        defer { busyPlace = nil }
        do {
            let update: ([Visit]) -> [Visit]
            // Visited today: undo that visit. Otherwise check in, also when there are older visits.
            let undoing = visited ?? place.visitedToday()
            if !undoing {
                var position: (lat: Double, lng: Double)?
                if let fix = gpsFix, Date().timeIntervalSince(fix.at) < 600 {
                    position = (fix.lat, fix.lng)
                }
                let visit = try await api.checkIn(placeID: place.id, here: position).visit
                // Checking in twice on one day returns that day's visit.
                update = { old in [visit] + old.filter { $0.id != visit.id } }
            } else if let latest = place.visits.first {
                try await api.removeVisit(id: latest.id)
                update = { old in old.filter { $0.id != latest.id } }
            } else {
                return
            }
            setVisits(place.id, update)
            // Details still loading may have read the visits from before this change.
            if let detail, detail.id == place.id, detailVisits != .loaded { openPlace(detail) }
            if undoing {
                // The server has the remaining visits; details shown here may be out of date.
                if let rest = try? await api.place(id: place.id, from: nil).visits {
                    setVisits(place.id) { _ in rest }
                } else if let detail, detail.id == place.id, detailVisits == .loaded,
                          let index = places.firstIndex(where: { $0.id == place.id }) {
                    // The card shows only the latest visit: fall back on the older ones in the details.
                    places[index].visits = Array(detail.visits.prefix(1))
                }
            }
            let after = try await api.discover(areaID: here.area.id)
            discover = after
            if !undoing { celebrate(before: before, after: after, place: place) }
        } catch {
            toast("That did not work: \(error.localizedDescription)")
        }
    }

    /// Reports a place as closed for good: it disappears from this user's lists right away.
    func reportClosed(_ place: Place) async {
        guard busyPlace == nil else { return }
        busyPlace = place.id
        defer { busyPlace = nil }
        do {
            try await api.reportClosed(placeID: place.id)
            places.removeAll { $0.id == place.id }
            if detail?.id == place.id { closePlace() }
            toast("Thanks for letting us know. \(place.name) is hidden for you now.")
            // The place no longer counts towards its countries.
            if let here = location { discover = try await api.discover(areaID: here.area.id) }
        } catch {
            toast("Could not report this place: \(error.localizedDescription)")
        }
    }

    /// Adds a restaurant OpenStreetMap lacks, at the device's position: the user stands at
    /// it, as OSM wants additions from people who know the place. The place, once added.
    func addPlace(name: String, country: String, address: String?, website: String?) async -> Place? {
        do {
            guard let fix = try? await locationService.current() else {
                toast("Allow location access and stand at the restaurant: its position is yours.")
                return nil
            }
            let lat = fix.coordinate.latitude, lng = fix.coordinate.longitude
            gpsFix = (lat, lng, Date())
            let place = try await api.addPlace(name: name, country: country, lat: lat, lng: lng, address: address, website: website)
            toast("\(place.name) added. You see it right away; OpenStreetMap mappers were asked to add it for everyone.")
            // It counts towards its country, for this user, if it lies in the area.
            if let here = location, let fresh = try? await api.discover(areaID: here.area.id) { discover = fresh }
            return place
        } catch {
            toast("Could not add the restaurant: \(error.localizedDescription)")
            return nil
        }
    }

    /// Updates a place's visits on its card (which shows only the latest) and in its details.
    private func setVisits(_ id: String, _ update: ([Visit]) -> [Visit]) {
        if let index = places.firstIndex(where: { $0.id == id }) {
            places[index].visits = Array(update(places[index].visits).prefix(1))
        }
        if detail?.id == id, let visits = detail?.visits {
            detail?.visits = update(visits)
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

enum DetailVisits { case loading, loaded, failed }
