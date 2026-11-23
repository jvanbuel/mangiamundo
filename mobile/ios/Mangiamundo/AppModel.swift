import Foundation
import GoogleSignIn
import Observation
import UIKit

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
    /// The place being read again from OpenStreetMap.
    private(set) var refreshingPlace: String?
    /// True while OpenStreetMap is asked for places just mapped nearby.
    private(set) var lookingNearby = false
    /// The place the user went to edit in an OpenStreetMap app, read again when they come back.
    @ObservationIgnored private var editing: String?
    /// A refresh waiting for the one running to finish.
    @ObservationIgnored private var queuedRefresh: String?
    /// The place whose details are open: its card's data until the full place has loaded.
    private(set) var detail: Place?
    /// Whether `detail` holds all visits yet.
    private(set) var detailVisits = DetailVisits.loading
    @ObservationIgnored private var detailTask: Task<Void, Never>?

    /// A spin is running: the globe turns and the Spin buttons wait.
    private(set) var spinning = false
    /// The place the last spin landed on, whose details offer to spin again.
    private(set) var spun: String?
    /// Places suggested since the area was picked, so spinning again finds another.
    @ObservationIgnored private var spunIDs: [String] = []
    /// How long the globe turns at least before a spin lands, and how long one turn takes.
    private static let spinSeconds = 1.6
    private static let notTodayKey = "mangiamundo.notToday"

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

    /// The account's recovery code status and devices, loaded when settings open.
    private(set) var account: Account?
    private(set) var accountError: String?
    private(set) var accountBusy = false
    /// Shown once, right after it is made.
    private(set) var recoveryCode: String?
    private(set) var linkCode: LinkCode?

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
        api = APIClient(baseURL: base ?? "http://localhost:8080", deviceName: UIDevice.current.model)
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
        spunIDs = []
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
                await announceConfirmedAdditions()
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

    /// Spins the globe: it lands on a country and opens an unvisited place there, open now if any is.
    func spin() async {
        guard let here = location, !spinning else { return }
        spinning = true
        defer { spinning = false }
        closePlace()
        let still = UIAccessibility.isReduceMotionEnabled
        let turning = still ? nil : Task { await turn() }
        defer { turning?.cancel() }
        let started = Date()
        let exclude = spunIDs + notToday.keys
        do {
            let found: Suggestion
            do {
                found = try await api.suggestion(areaID: here.area.id, from: userPosition, exclude: exclude, open: true)
            } catch let error as APIError where error.code == "nothing_open" {
                toast("Nothing open right now. Here is one for another time.")
                found = try await api.suggestion(areaID: here.area.id, from: userPosition, exclude: exclude, open: false)
            }
            if !still {
                let left = Self.spinSeconds - Date().timeIntervalSince(started)
                if left > 0 { try? await Task.sleep(for: .seconds(left)) }
            }
            turning?.cancel()
            await turning?.value
            guard location?.area.id == here.area.id else { return }
            spunIDs.append(found.place.id)
            spun = found.place.id
            select(found.country)
            if let country = world?.countries.first(where: { $0.code == found.country }) {
                await glide(to: country.landing(), still: still)
            }
            openPlace(found.place)
        } catch let error as APIError where error.code == "nothing_left" {
            if spunIDs.isEmpty {
                toast("You have been to every place in \(here.area.name).")
            } else {
                spunIDs = []
                toast("That was every place. Spin again to start over.")
            }
        } catch {
            toast("Could not spin: \(error.localizedDescription)")
        }
    }

    /// "Not today": keeps the place out of spins for 20 hours, and spins again.
    func notToday(_ placeID: String) async {
        var all = notToday
        all[placeID] = Date().addingTimeInterval(20 * 3600).timeIntervalSince1970
        defaults.set(all, forKey: Self.notTodayKey)
        await spin()
    }

    /// Places put off with "Not today", by id, with when they come back (seconds since 1970).
    private var notToday: [String: Double] {
        let all = defaults.dictionary(forKey: Self.notTodayKey) as? [String: Double] ?? [:]
        let now = Date().timeIntervalSince1970
        return all.filter { $0.value > now }
    }

    /// Turns the globe one turn per `spinSeconds`, zoomed out to all of it, until cancelled.
    private func turn() async {
        let from = camera
        let started = Date()
        while !Task.isCancelled {
            let seconds = Date().timeIntervalSince(started)
            let out = min(1, seconds / 0.5)
            camera = Camera(lng: from.lng + 360 * seconds / Self.spinSeconds, lat: from.lat, zoom: from.zoom + (1 - from.zoom) * out).normalized()
            try? await Task.sleep(for: .milliseconds(16))
        }
    }

    /// Moves the camera to `to`, slowing down at the end; the short way around the globe.
    private func glide(to target: Camera, still: Bool) async {
        guard !still else {
            camera = target
            return
        }
        let from = camera
        let delta = (target.lng - from.lng + 540).truncatingRemainder(dividingBy: 360) - 180
        let started = Date()
        while true {
            let t = min(1, Date().timeIntervalSince(started) / 0.9)
            let e = 1 - pow(1 - t, 3)
            camera = Camera(lng: from.lng + delta * e, lat: from.lat + (target.lat - from.lat) * e, zoom: from.zoom + (target.zoom - from.zoom) * e).normalized()
            if t >= 1 { return }
            try? await Task.sleep(for: .milliseconds(16))
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

    func editingOnOsm(_ placeID: String) {
        editing = placeID
    }

    /// Back in the app: shows an edit just made to a place on OpenStreetMap.
    func onActive() async {
        guard let id = editing else { return }
        editing = nil
        // A refresh still running may predate the edit: this one goes after it.
        if refreshingPlace != nil { queuedRefresh = id } else { await refreshPlace(id, quiet: true) }
    }

    /// Reads a place again from OpenStreetMap and says what changed. With `quiet`, a failure
    /// isn't shown, as when it runs by itself after the user comes back from editing.
    func refreshPlace(_ placeID: String, quiet: Bool = false) async {
        guard refreshingPlace == nil else { return }
        refreshingPlace = placeID
        let name = (detail?.id == placeID ? detail?.name : nil) ?? places.first(where: { $0.id == placeID })?.name ?? "This place"
        do {
            let result = try await api.refreshPlace(id: placeID, from: userPosition)
            if result.status == .gone || result.place == nil {
                places.removeAll { $0.id == placeID }
                if detail?.id == placeID { closePlace() }
                toast("\(name) is gone from OpenStreetMap, so it's hidden now. Your visits to it are kept.")
            } else if let place = result.place {
                // Also when unchanged: someone else's refresh may have updated it since it loaded.
                applyRefreshed(place)
                toast(result.status == .updated
                    ? "\(place.name) updated from OpenStreetMap."
                    : "No change on OpenStreetMap yet. A new edit can take a minute or two to show up there.")
            }
            // Its countries may have changed, also with an unchanged reply when someone else's
            // refresh changed it first. A reply for an area the user has left is dropped.
            if let here = location {
                let refresh = refreshes
                if let fresh = try? await api.discover(areaID: here.area.id), refresh == refreshes { discover = fresh }
            }
        } catch {
            if !quiet { toast("Could not refresh this place: \(error.localizedDescription)") }
        }
        refreshingPlace = nil
        if let next = queuedRefresh {
            queuedRefresh = nil
            await refreshPlace(next, quiet: true)
        }
    }

    /// Shows a place read again from OpenStreetMap. Visits stay as shown: a check-in or undo
    /// made while the refresh ran is newer than the reply. A place no longer serving the
    /// selected country's food leaves that list.
    private func applyRefreshed(_ place: Place) {
        if let current = detail, current.id == place.id {
            var fresh = place
            fresh.visits = current.visits
            detail = fresh
        }
        if let index = places.firstIndex(where: { $0.id == place.id }) {
            if let code = selected, !place.countries.contains(code) {
                places.remove(at: index)
            } else {
                var card = place
                card.visits = places[index].visits
                places[index] = card
            }
        }
    }

    /// Asks OpenStreetMap for restaurants just mapped around the user, for someone who added
    /// one in an OpenStreetMap app instead of the add form.
    func lookAgainNearby() async {
        guard !lookingNearby else { return }
        lookingNearby = true
        defer { lookingNearby = false }
        do {
            let fix = try await locationService.current()
            let lat = fix.coordinate.latitude, lng = fix.coordinate.longitude
            gpsFix = (lat, lng, Date())
            let result = try await api.refreshNearby(lat: lat, lng: lng)
            let added = result.added
            if added.isEmpty && result.matched == 0 {
                toast("Nothing new within 150 m yet. A place just added on OpenStreetMap can take a minute or two to show up.")
            } else {
                if !added.isEmpty { toast("Found \(added.map(\.name).joined(separator: ", ")): on the map for everyone now.") }
                // New countries, and any of the user's additions they confirmed.
                refresh()
                // The open country's list, with the places just found.
                if let code = selected { select(code) }
            }
        } catch {
            toast("Could not look again: \(error.localizedDescription)")
        }
    }

    /// Where the add form's pin starts: the device's position, or the area's centre without one.
    func startingPoint() async -> (lat: Double, lng: Double) {
        if let fix = try? await locationService.current() {
            let lat = fix.coordinate.latitude, lng = fix.coordinate.longitude
            gpsFix = (lat, lng, Date())
            return (lat, lng)
        }
        return userPosition ?? (location?.lat ?? 0, location?.lng ?? 0)
    }

    /// Adds a restaurant OpenStreetMap lacks, where the user put the pin. The place, once added.
    /// `note: false` when the user adds it to OSM themselves, so no note asks mappers to.
    func addPlace(name: String, country: String, lat: Double, lng: Double, address: String?, website: String?, note: Bool = true) async -> Place? {
        do {
            let added = try await api.addPlace(name: name, country: country, lat: lat, lng: lng, address: address, website: website, note: note)
            let place = added.place
            let earns = added.reward.pendingText(country: country.uppercased(), catalog: catalog)
            toast(note
                ? "\(place.name) added. You see it right away. \(earns)"
                : "\(place.name) added for you. Now add it to the map in the other app, so everyone gets it. \(earns)")
            // It counts towards its country, for this user, if it lies in the area.
            // Applied only if no refresh (such as a change of area) started meanwhile.
            let refresh = refreshes
            if let here = location, let fresh = try? await api.discover(areaID: here.area.id), refresh == refreshes {
                discover = fresh
            }
            return place
        } catch {
            toast("Could not add the restaurant: \(error.localizedDescription)")
            return nil
        }
    }

    /// "Lalibela is on the map. +130 XP…" for each addition OpenStreetMap now has that the
    /// user wasn't told about. A failure leaves them for the next refresh.
    private func announceConfirmedAdditions() async {
        // A few at a time: any more wait for the next refresh.
        guard let all = try? await api.additions() else { return }
        let news = Array(all.filter(\.isNews).prefix(10))
        guard !news.isEmpty else { return }
        for addition in news { toast(addition.announcement(catalog: catalog)) }
        try? await api.markAdditionsSeen(ids: news.map(\.id))
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
            guard now.status != was.status, now.status != .available else { continue }
            let more = now.status == .completed ? "" : " \(now.goal - now.visited) more for diamond."
            toast("\(info.flag) \(info.name): \(now.status.label.lowercased()).\(more)")
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

    // MARK: - Account

    func loadAccount() async {
        do {
            account = try await api.account()
            accountError = nil
        } catch {
            accountError = error.localizedDescription
        }
    }

    /// Runs one account action at a time, reporting failures in a toast.
    private func accountAction(_ work: @escaping @MainActor () async throws -> Void) {
        guard !accountBusy else { return }
        accountBusy = true
        Task {
            do {
                try await work()
            } catch {
                toast("That did not work: \(error.localizedDescription)")
            }
            accountBusy = false
        }
    }

    func makeRecoveryCode() {
        accountAction {
            self.recoveryCode = try await self.api.newRecoveryCode()
            self.account = try await self.api.account()
        }
    }

    /// Whether a share sheet is being prepared.
    private(set) var sharing = false

    /// Shares the map of the current area through a public page, made on first use.
    func shareProgress() {
        guard let areaID = location?.area.id else { return }
        share { try await self.api.shareProgress(areaID: areaID) }
    }

    func shareVisit(_ id: Int) {
        share { try await self.api.shareVisit(id: id) }
    }

    private func share(_ make: @escaping @MainActor () async throws -> ShareLink) {
        guard !sharing else { return }
        sharing = true
        Task {
            defer { sharing = false }
            do {
                let link = try await make()
                var items: [Any] = []
                if let data = await api.card(link), let image = UIImage(data: data) { items.append(image) }
                items.append(link.text)
                if let url = URL(string: link.url) { items.append(url) }
                // The settings load the account, public link included, when they open.
                presentShareSheet(items)
            } catch {
                toast("Could not share: \(error.localizedDescription)")
            }
        }
    }

    /// The public page as a full link; nil when it's off.
    var publicProfileURL: URL? {
        account?.publicProfile.flatMap { URL(string: api.baseURL + $0) }
    }

    func unshareProgress() {
        accountAction {
            try await self.api.unshareProgress()
            self.account = try await self.api.account()
        }
    }

    func makeLinkCode() {
        accountAction { self.linkCode = try await self.api.newLinkCode() }
    }

    func clearLinkCode() {
        linkCode = nil
    }

    /// Signs in to the account of a recovery or link code; what this device did moves into it.
    func restore(code: String, onRestored: @escaping @MainActor () -> Void) {
        accountAction {
            do {
                try await self.api.restore(code: code)
            } catch let error as APIError where error.status == 404 {
                self.toast("Code not recognised. A link code works once, for 10 minutes.")
                return
            }
            self.recoveryCode = nil
            self.linkCode = nil
            onRestored()
            self.toast("Welcome back. Your progress is on this device now.")
            await self.accountChanged()
        }
    }

    func signOut(deviceID: String) {
        accountAction {
            try await self.api.removeDevice(id: deviceID)
            self.account = try await self.api.account()
        }
    }

    /// The address a sign-in code went to, while the code is awaited.
    private(set) var codeSentTo: String?

    func sendEmailCode(_ email: String) {
        accountAction {
            let address = email.trimmingCharacters(in: .whitespaces)
            try await self.api.sendEmailCode(address)
            self.codeSentTo = address
        }
    }

    func forgetEmailCode() {
        codeSentTo = nil
    }

    func signIn(code: String) {
        guard let email = codeSentTo else { return }
        accountAction {
            do {
                try await self.api.signIn(email: email, code: code)
            } catch let error as APIError where error.status == 404 {
                self.toast("Wrong code. Check it, or ask for a new one.")
                return
            }
            self.codeSentTo = nil
            await self.signedIn()
        }
    }

    func signInWithApple(idToken: String) {
        accountAction {
            try await self.api.signInWithApple(idToken: idToken)
            await self.signedIn()
        }
    }

    /// Opens Google's sign-in sheet and signs in with the ID token it hands back.
    func signInWithGoogle() {
        accountAction {
            let window = UIApplication.shared.connectedScenes.compactMap { ($0 as? UIWindowScene)?.keyWindow }.first
            guard var top = window?.rootViewController else { return }
            while let presented = top.presentedViewController { top = presented }
            let result: GIDSignInResult
            do {
                result = try await GIDSignIn.sharedInstance.signIn(withPresenting: top)
            } catch let error as GIDSignInError where error.code == .canceled {
                return
            }
            guard let token = result.user.idToken?.tokenString else {
                self.toast("Google did not hand back a sign-in token. Try again.")
                return
            }
            try await self.api.signInWithGoogle(idToken: token)
            await self.signedIn()
        }
    }

    private func signedIn() async {
        recoveryCode = nil
        linkCode = nil
        toast("Signed in. Your progress now follows you to every device you sign in on.")
        await accountChanged()
    }

    func signOut() {
        accountAction {
            try await self.api.signOut()
            self.recoveryCode = nil
            self.linkCode = nil
            self.toast("Signed out. Sign in again to get your progress back here.")
            await self.accountChanged()
        }
    }

    func deleteAccount() {
        accountAction {
            try await self.api.deleteAccount()
            self.recoveryCode = nil
            self.linkCode = nil
            self.toast("Account deleted. You start fresh on this device.")
            await self.accountChanged()
        }
    }

    /// After this device switches account: drop the previous account's visits, then load this one's.
    private func accountChanged() async {
        detailTask?.cancel()
        detail = nil
        // Cancels the places loader, clears its places and loads the open country again.
        select(selected)
        refresh()
        // A failure here only shows in the account section: the switch itself worked.
        await loadAccount()
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
