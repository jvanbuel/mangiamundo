import Foundation
import UIKit

/// Sharing public pages of the user's progress and check-ins.
extension AppModel {
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
}
