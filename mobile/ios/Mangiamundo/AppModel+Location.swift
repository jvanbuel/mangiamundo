import Foundation

/// Where the user explores: the area, recent areas and the device's position.
extension AppModel {
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
}
