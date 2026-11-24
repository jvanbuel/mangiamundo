import SwiftUI

extension BadgeKind {
    var symbol: String {
        switch self {
        case .visits: return "fork.knife"
        case .countries: return "flag.fill"
        case .completed: return "checkmark.seal.fill"
        case .continent: return "map.fill"
        case .world: return "globe.europe.africa.fill"
        case .everything: return "crown.fill"
        case .pioneer: return "mappin.and.ellipse"
        case .streak: return "flame.fill"
        case .unknown: return "star.fill"
        }
    }

    /// Gold for tasting, diamond for finishing, blue for the rest.
    var face: Medal.Face {
        switch self {
        case .visits, .countries, .streak: return .gold
        case .completed, .continent, .world, .everything: return .diamond
        case .pioneer, .unknown: return .blue
        }
    }
}

/// A solid token with a darker lower edge, so it reads as something you collect.
struct Medal: View {
    enum Face { case gold, diamond, blue, event }

    let symbol: String
    let face: Face
    var earned = true
    var size: CGFloat = 64

    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var glint = false

    init(kind: BadgeKind, earned: Bool = true, size: CGFloat = 64) {
        self.init(symbol: kind.symbol, face: kind.face, earned: earned, size: size)
    }

    init(symbol: String, face: Face, earned: Bool = true, size: CGFloat = 64) {
        self.symbol = symbol
        self.face = face
        self.earned = earned
        self.size = size
    }

    /// The medal of a limited-time event.
    static func event(earned: Bool = true, size: CGFloat = 64) -> Medal {
        Medal(symbol: "party.popper.fill", face: .event, earned: earned, size: size)
    }

    private var fill: AnyShapeStyle {
        guard earned else { return AnyShapeStyle(Theme.line) }
        switch face {
        case .gold: return AnyShapeStyle(Theme.discovered)
        case .diamond: return AnyShapeStyle(Theme.diamond)
        case .blue: return AnyShapeStyle(Theme.accent)
        case .event: return AnyShapeStyle(Theme.event)
        }
    }

    private var edge: Color {
        guard earned else { return Theme.line }
        switch face {
        case .gold: return Theme.discoveredEdge
        case .diamond: return Theme.completedEdge
        case .blue, .event: return Theme.accent
        }
    }

    private var ink: Color {
        guard earned else { return Theme.muted }
        switch face {
        case .gold: return Theme.onDiscovered
        case .diamond: return Theme.onCompleted
        case .blue, .event: return Theme.onAccent
        }
    }

    var body: some View {
        let darken = earned && (face == .blue || face == .event) || !earned
        ZStack {
            Circle().fill(edge).brightness(darken ? -0.18 : 0)
            Circle().fill(fill).padding(.bottom, size * 0.07)
            if earned && face == .diamond {
                // Diamond medals catch the light now and then: a glint sweeps across.
                LinearGradient(colors: [.clear, .white.opacity(0.9), .clear], startPoint: .leading, endPoint: .trailing)
                    .frame(width: size * 0.5)
                    .rotationEffect(.degrees(20))
                    .offset(x: glint ? size : -size)
                    .mask(Circle().padding(.bottom, size * 0.07))
                    .onAppear {
                        guard !reduceMotion else { return }
                        withAnimation(.easeInOut(duration: 1.2).delay(2.8).repeatForever(autoreverses: false)) { glint = true }
                    }
            }
            Image(systemName: symbol)
                .font(.system(size: size * 0.4, weight: .bold))
                .foregroundStyle(ink)
                .offset(y: -size * 0.035)
        }
        .frame(width: size, height: size)
        .accessibilityHidden(true)
    }
}
