import Foundation
import UIKit

/// Spinning the globe for a place to try.
extension AppModel {
    /// How long the globe turns at least before a spin lands, and how long one turn takes.
    private static let spinSeconds = 1.6
    private static let notTodayKey = "mangiamundo.notToday"

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
}
