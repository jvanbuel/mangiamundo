import Foundation

/// Public pages of the user's progress and check-ins.
extension APIClient {
    /// Makes (or moves to this area) the public page of the user's progress.
    func shareProgress(areaID: String) async throws -> ShareLink {
        try decode(await write("POST", "/api/share?\(area(areaID))"))
    }

    /// Turns the public page off.
    func unshareProgress() async throws {
        try await writeMine("DELETE", "/api/share")
    }

    /// A public page for one check-in.
    func shareVisit(id: Int) async throws -> ShareLink {
        try decode(await write("POST", "/api/visits/\(id)/share"))
    }

    /// The share card; nil when it can't be loaded, so the link is shared alone.
    func card(_ link: ShareLink) async -> Data? {
        guard let url = URL(string: link.imageUrl),
              let (data, response) = try? await URLSession.shared.data(from: url),
              (response as? HTTPURLResponse)?.statusCode == 200 else { return nil }
        return data
    }
}
