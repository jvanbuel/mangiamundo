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
    var location: SavedLocation?
    /// Areas chosen before, newest first, to switch back to from the location picker.
    var recentAreas: [Area] = []
    var discover: Discover?
    var loading = false
    /// How far loading the area has come, while that takes long enough to show a wheel.
    var areaProgress: AreaProgress?
    var error: String?
    /// The first data is in (or failed), so the startup loader can go.
    var ready = false

    /// Code of the country whose places are shown.
    var selected: String?
    var places: [Place] = []
    var placesLoading = false
    var busyPlace: String?
    /// The place being read again from OpenStreetMap.
    var refreshingPlace: String?
    /// True while OpenStreetMap is asked for places just mapped nearby.
    var lookingNearby = false
    /// The place the user went to edit in an OpenStreetMap app, read again when they come back.
    @ObservationIgnored var editing: String?
    /// A refresh waiting for the one running to finish.
    @ObservationIgnored var queuedRefresh: String?
    /// The place whose details are open: its card's data until the full place has loaded.
    var detail: Place?
    /// Whether `detail` holds all visits yet.
    var detailVisits = DetailVisits.loading
    @ObservationIgnored var detailTask: Task<Void, Never>?

    /// A spin is running: the globe turns and the Spin buttons wait.
    var spinning = false
    /// The place the last spin landed on, whose details offer to spin again.
    var spun: String?
    /// Places suggested since the area was picked, so spinning again finds another.
    @ObservationIgnored var spunIDs: [String] = []

    var locating = false
    var locateError: String?

    var celebrations: [Celebration] = []
    var remindersOn = false
    /// 0 = countries, 1 = badges.
    var tab = 0
    /// The server's version and commit; nil until it answers, "not reached" after a failure.
    var server: String?

    private(set) var projection = ProjectionKind.globe
    private(set) var directionsWith = DirectionsPreference.ask
    var camera = Camera()
    /// Short announcements (badges, discoveries, errors), newest last.
    private(set) var toasts: [Toast] = []

    /// The account's recovery code status and devices, loaded when settings open.
    var account: Account?
    var accountError: String?
    var accountBusy = false
    /// Shown once, right after it is made.
    var recoveryCode: String?
    var linkCode: LinkCode?
    /// Whether a share sheet is being prepared.
    var sharing = false
    /// The address a sign-in code went to, while the code is awaited.
    var codeSentTo: String?
    /// Goes up with every change of account, so an answer for the previous one can be told apart.
    var accountGeneration = 0

    @ObservationIgnored let api: APIClient
    @ObservationIgnored let locationService = LocationService()
    @ObservationIgnored let defaults = UserDefaults.standard
    /// The last real device position, used to verify check-ins while browsing another city.
    @ObservationIgnored var gpsFix: (lat: Double, lng: Double, at: Date)?
    @ObservationIgnored var refreshTask: Task<Void, Never>?
    /// Counts refreshes, so only the latest one shows or clears the progress wheel.
    @ObservationIgnored var refreshes = 0
    @ObservationIgnored var placesTask: Task<Void, Never>?
    @ObservationIgnored private var started = false
    @ObservationIgnored let reminders = ReminderCenter()
    /// A country to open once progress has loaded, from a notification tap.
    @ObservationIgnored var pendingCountry: String?

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
        Task { [weak self, events = api.signedOut] in
            for await _ in events { await self?.signedOut() }
        }
        loadServer()
        do {
            catalog = try await api.catalog()
        } catch {
            self.error = error.localizedDescription
            ready = true
            return
        }
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

    /// Where the user is, when known recently: distances are measured from it.
    var userPosition: (lat: Double, lng: Double)? {
        if let fix = gpsFix, Date().timeIntervalSince(fix.at) < 600 { return (fix.lat, fix.lng) }
        guard let location, location.fromGPS else { return nil }
        return (location.lat, location.lng)
    }

    static let recentKey = "mangiamundo.recentAreas"

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
