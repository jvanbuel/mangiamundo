import SwiftUI

struct Passport: View {
    let discover: Discover
    @Environment(AppModel.self) private var model

    var body: some View {
        let profile = discover.profile
        Card {
            VStack(alignment: .leading, spacing: 14) {
                VStack(spacing: 6) {
                    HStack(alignment: .firstTextBaseline) {
                        Text("Level \(profile.level)").font(Theme.font(.subheadline, .bold))
                        Spacer()
                        Text("\(profile.xp) / \(profile.nextLevelXp) XP")
                            .font(Theme.font(.subheadline))
                            .monospacedDigit()
                            .foregroundStyle(Theme.muted)
                    }
                    Bar(part: profile.xp - profile.levelXp, whole: profile.nextLevelXp - profile.levelXp)
                }

                VStack(spacing: 6) {
                    HStack(alignment: .firstTextBaseline) {
                        Text("Countries tasted").font(Theme.font(.subheadline, .bold))
                        Spacer()
                        (Text("\(discover.world.discovered)").font(Theme.font(.subheadline, .extraBold))
                            + Text(" of \(discover.world.available)").foregroundColor(Theme.muted))
                            .font(Theme.font(.subheadline))
                            .monospacedDigit()
                    }
                    Bar(part: discover.world.discovered, whole: discover.world.available)
                }

                Divider().overlay(Theme.line)
                HStack(alignment: .top) {
                    stat("Places visited", "\(discover.placesVisited) of \(discover.places)")
                    stat("Diamond countries", "\(discover.world.completed)")
                    stat("Month streak", "\(profile.streak ?? 0) · best \(profile.bestStreak ?? 0)")
                }

                if discover.placesVisited > 0 {
                    HStack(spacing: 10) {
                        Button { model.shareProgress() } label: {
                            Label(model.sharing ? "Preparing…" : "Share my map", systemImage: "square.and.arrow.up")
                        }
                        .buttonStyle(ChunkyButtonStyle(small: true))
                        .disabled(model.sharing)
                        Text("Makes a public link to your map, level and badges.")
                            .font(Theme.font(.caption))
                            .foregroundStyle(Theme.muted)
                    }
                }
            }
        }
    }

    private func stat(_ label: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label).font(Theme.font(.footnote)).foregroundStyle(Theme.muted)
            Text(value).font(Theme.font(.subheadline, .bold)).monospacedDigit()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}
