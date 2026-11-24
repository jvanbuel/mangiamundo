import SwiftUI

struct Header: View {
    @Environment(AppModel.self) private var model
    let onChange: () -> Void
    let onSettings: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Mangiamundo")
                        .font(Theme.font(.title2, .black))
                    Text("Eat the world, start with your city")
                        .font(Theme.font(.subheadline, .bold))
                        .foregroundStyle(Theme.muted)
                }
                Spacer()
                Button(action: onSettings) {
                    Image(systemName: "gearshape").font(Theme.font(.title3)).foregroundStyle(Theme.muted)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Settings")
            }
            if let location = model.location {
                Button(action: onChange) {
                    HStack(spacing: 6) {
                        Image(systemName: location.fromGPS ? "location.fill" : "mappin")
                            .font(Theme.font(.footnote))
                        Text(location.area.name)
                            .font(Theme.font(.body, .bold))
                            .lineLimit(1)
                        if location.area.kind == .postalCode {
                            Text("· postal code").foregroundStyle(Theme.muted)
                        }
                        Spacer()
                        Text("Change").font(Theme.font(.footnote)).foregroundStyle(Theme.accent)
                    }
                    .padding(.horizontal, 12)
                    .padding(.vertical, 10)
                    .overlay(RoundedRectangle(cornerRadius: 10).stroke(Theme.line))
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
        }
    }
}
