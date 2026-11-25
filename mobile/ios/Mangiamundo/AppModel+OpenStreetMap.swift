import Foundation

/// Reading places again from OpenStreetMap after the user edits or adds them there.
extension AppModel {
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
}
