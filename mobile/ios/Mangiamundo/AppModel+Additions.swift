import Foundation

/// Restaurants the user adds, and food the user names for places OpenStreetMap gives none.
extension AppModel {
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
            let earns = added.reward.pendingText(country: country.uppercased(), catalog: catalog)
            toast(note
                ? "\(added.place.name) added. You see it right away. \(earns)"
                : "\(added.place.name) added for you. Now add it to the map in the other app, so everyone gets it. \(earns)")
            await countIn()
            return added.place
        } catch {
            toast("Could not add the restaurant: \(error.localizedDescription)")
            return nil
        }
    }

    /// Refreshes the progress after a pending place was added: it counts towards its country,
    /// for this user, if it lies in the area. Applied only if no refresh (such as a change of
    /// area) started meanwhile.
    private func countIn() async {
        let refresh = refreshes
        if let here = location, let fresh = try? await api.discover(areaID: here.area.id), refresh == refreshes {
            discover = fresh
        }
    }

    /// Restaurants in the area with unknown food, nearest to the user first. Empty on failure.
    func unknownPlaces() async -> [UnknownPlace] {
        guard let here = location else { return [] }
        return (try? await api.unknownPlaces(areaID: here.area.id, from: userPosition)) ?? []
    }

    /// "It serves food from …" for a restaurant with unknown food. The place, once saved.
    func suggestCuisine(_ unknown: UnknownPlace, country: String) async -> Place? {
        do {
            let added = try await api.suggestCuisine(placeID: unknown.id, country: country)
            let earns = added.reward.pendingText(country: country.uppercased(), catalog: catalog)
            toast("\(added.place.name) added. You see it right away. \(earns)")
            await countIn()
            return added.place
        } catch {
            toast("Could not save that: \(error.localizedDescription)")
            return nil
        }
    }

    /// "It serves no particular country's food". True once saved.
    func noCuisine(_ unknown: UnknownPlace) async -> Bool {
        do {
            try await api.noCuisine(placeID: unknown.id)
            return true
        } catch {
            toast("Could not save that: \(error.localizedDescription)")
            return false
        }
    }

    /// "Lalibela is on the map. +130 XP…" for each addition OpenStreetMap now has that the
    /// user wasn't told about. A failure leaves them for the next refresh.
    func announceConfirmedAdditions() async {
        // A few at a time: any more wait for the next refresh.
        guard let all = try? await api.additions() else { return }
        let news = Array(all.filter(\.isNews).prefix(10))
        guard !news.isEmpty else { return }
        for addition in news { toast(addition.announcement(catalog: catalog)) }
        try? await api.markAdditionsSeen(ids: news.map(\.id))
    }
}
