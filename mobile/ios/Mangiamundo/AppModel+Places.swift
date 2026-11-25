import Foundation

/// The selected country's places, a place's details, check-ins and closed reports.
extension AppModel {
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
                let result: [Place]
                if elsewhere(code) {
                    result = try await api.nearest(country: code, from: userPosition ?? (lat: here.lat, lng: here.lng))
                } else {
                    result = try await api.places(areaID: here.area.id, country: code, from: userPosition)
                }
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

    /// A country the area has no place of, once the area's countries are known. Its list is
    /// then the nearest places elsewhere, measured from the user or else the map pin.
    func elsewhere(_ code: String) -> Bool {
        guard let discover else { return false }
        return !discover.countries.contains { $0.code == code }
    }

    /// A tap on the map: open the country, with its nearest places elsewhere when the area has none.
    func tapCountry(code: String, name: String) {
        guard discover != nil else { return }
        if country(code) != nil {
            select(code)
        } else {
            toast("The game has no cuisine for \(name) yet.")
        }
    }

    /// Checks in, or undoes today's visit. `visited` is what the button showed, so the
    /// action matches its label even if midnight passes in between.
    func toggleVisit(_ place: Place, visited: Bool? = nil) async {
        guard discover != nil, let here = location, busyPlace == nil else { return }
        busyPlace = place.id
        defer { busyPlace = nil }
        let generation = accountGeneration
        // Nothing of the answer is shown once the device is in another account.
        let sameAccount = { self.accountGeneration == generation }
        // Progress is only shown when it is also still about this area.
        let stillHere = { self.location?.area.id == here.area.id && sameAccount() }
        do {
            let update: ([Visit]) -> [Visit]
            var checkedIn: VisitResult?
            var progress: Discover?
            // Visited today: undo that visit. Otherwise check in, also when there are older visits.
            let undoing = visited ?? place.visitedToday()
            if !undoing {
                var position: (lat: Double, lng: Double)?
                if let fix = gpsFix, Date().timeIntervalSince(fix.at) < 600 {
                    position = (fix.lat, fix.lng)
                }
                let result = try await api.checkIn(placeID: place.id, areaID: here.area.id, here: position)
                checkedIn = result
                progress = result.progress
                let visit = result.visit
                // Checking in twice on one day returns that day's visit.
                update = { old in [visit] + old.filter { $0.id != visit.id } }
            } else if let latest = place.visits.first {
                let result: VisitRemoved
                do {
                    result = try await api.removeVisit(id: latest.id, areaID: here.area.id)
                } catch let error as APIError where error.code == "not_found" {
                    // Gone already, from another device or by a retry whose answer was lost: ask what is left.
                    async let rest = api.place(id: place.id, from: nil)
                    async let now = api.discover(areaID: here.area.id)
                    let (left, after) = try await (rest, now)
                    result = VisitRemoved(visits: left.visits, profile: after.profile, progress: after)
                }
                progress = result.progress
                // The server's list of the visits left: the card only knew the latest.
                update = { _ in result.visits }
            } else {
                return
            }
            guard sameAccount() else { return }
            setVisits(place.id, update)
            // Details still loading may have read the visits from before this change.
            if let detail, detail.id == place.id, detailVisits != .loaded { openPlace(detail) }
            if let progress, stillHere() { discover = progress }
            for unlocked in checkedIn?.unlocked ?? [] { toast(unlocked.announcement(catalog: catalog)) }
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
            let here = location
            let generation = accountGeneration
            let result = try await api.reportClosed(placeID: place.id, areaID: here?.area.id)
            guard accountGeneration == generation else { return }
            places.removeAll { $0.id == place.id }
            if detail?.id == place.id { closePlace() }
            toast("Thanks for letting us know. \(place.name) is hidden for you now.")
            // The place no longer counts towards its countries.
            if let progress = result.progress, let here, location?.area.id == here.area.id { discover = progress }
        } catch {
            toast("Could not report this place: \(error.localizedDescription)")
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
}
