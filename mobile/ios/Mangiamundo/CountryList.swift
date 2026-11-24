import SwiftUI

struct CountryList: View {
    @Environment(AppModel.self) private var model
    let discover: Discover

    var body: some View {
        ForEach(discover.continents.filter { $0.available > 0 }) { continent in
            VStack(alignment: .leading, spacing: 0) {
                HStack {
                    Text(continent.name).font(Theme.font(.headline, .extraBold))
                    Spacer()
                    Text("\(continent.discovered)/\(continent.available)")
                        .font(Theme.font(.subheadline))
                        .monospacedDigit()
                        .foregroundStyle(Theme.muted)
                }
                .padding(.horizontal, 8)
                .padding(.bottom, 4)

                ForEach(rows(in: continent)) { entry in
                    let info = model.country(entry.code)
                    Button {
                        model.select(entry.code)
                    } label: {
                        HStack(spacing: 10) {
                            Text(info?.flag ?? "").font(.title3).frame(width: 30)
                            Text(info?.name ?? entry.code)
                            Spacer()
                            Text("\(entry.visited)/\(entry.places)")
                                .font(Theme.font(.footnote))
                                .monospacedDigit()
                                .foregroundStyle(Theme.muted)
                            StatusMark(status: entry.status)
                        }
                        .padding(8)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
            }
        }
    }

    private func rows(in continent: GroupProgress) -> [CountryProgress] {
        discover.countries
            .filter { $0.continent == continent.code }
            .sorted { (model.country($0.code)?.name ?? $0.code) < (model.country($1.code)?.name ?? $1.code) }
    }
}
