import Foundation
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
