import SwiftUI

/// The lit-up map with a caption underneath, rendered only once the share sheet asks for it.
struct ShareCard: Transferable {
    let world: World
    let statuses: [String: Status]
    let kind: ProjectionKind
    let camera: Camera
    let headline: String
    let caption: String

    static var transferRepresentation: some TransferRepresentation {
        DataRepresentation(exportedContentType: .png) { card in
            try await card.png()
        }
        .suggestedFileName("mangiamundo.png")
    }

    @MainActor private func png() throws -> Data {
        let renderer = ImageRenderer(content: content)
        renderer.scale = 2
        guard let data = renderer.uiImage?.pngData() else { throw CocoaError(.fileWriteUnknown) }
        return data
    }

    /// 600 × 315 points: the 1.91:1 shape link previews and feeds use.
    private var content: some View {
        VStack(alignment: .leading, spacing: 0) {
            WorldMapView(
                world: world, statuses: statuses, selected: nil, here: nil, kind: kind,
                camera: .constant(camera), onTap: { _ in }
            )
            .frame(width: 600, height: 240)
            .background(Theme.bg)
            VStack(alignment: .leading, spacing: 2) {
                Text(headline).font(.custom(Theme.Weight.extraBold.rawValue, size: 26)).foregroundStyle(Theme.text)
                Text(caption).font(.custom(Theme.Weight.semibold.rawValue, size: 15)).foregroundStyle(Theme.muted)
            }
            .padding(.horizontal, 24)
            .frame(width: 600, height: 75, alignment: .leading)
            .background(Theme.panel)
        }
    }
}
