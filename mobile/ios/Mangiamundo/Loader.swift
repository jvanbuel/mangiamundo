import SwiftUI

/// Startup loader: picks up where the launch screen (the whole globe) leaves off and takes
/// the three bites of the app icon. Calls `onDone` once `ready`, never halfway through a bite.
struct Loader: View {
    let ready: Bool
    let onDone: () -> Void

    /// The bite circles (cx, cy, r) on the icon's 512-point canvas: tools/build-logo.mjs BITES.
    private static let bites: [(CGFloat, CGFloat, CGFloat)] = [(398, 118, 62), (318, 92, 46), (436, 196, 44)]
    /// The launch screen shows LaunchGlobe at its 200-point size.
    private static let size: CGFloat = 200

    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var grown = [false, false, false]
    @State private var biting = false
    @State private var squash = false
    @State private var pulse = false

    var body: some View {
        let scale = Self.size / 512
        ZStack {
            Color("LaunchBackground")
            ZStack(alignment: .topLeading) {
                Image("LaunchGlobe").resizable()
                ForEach(0..<3, id: \.self) { i in
                    let (cx, cy, r) = Self.bites[i]
                    Circle()
                        .fill(Color("LaunchBackground"))
                        .frame(width: 2 * r * scale, height: 2 * r * scale)
                        .scaleEffect(grown[i] ? 1 : 0)
                        .position(x: cx * scale, y: cy * scale)
                }
            }
            .frame(width: Self.size, height: Self.size)
            .scaleEffect(x: squash ? 1.03 : 1, y: squash ? 0.97 : 1)
            .scaleEffect(pulse ? 1.04 : 1)
        }
        // Centred on the whole screen, like the launch screen image.
        .ignoresSafeArea()
        .accessibilityHidden(true)
        .onChange(of: ready && !biting, initial: true) { _, go in if go { onDone() } }
        .task { try? await bite() }
    }

    private func bite() async throws {
        if reduceMotion {
            grown = [true, true, true]
            return
        }
        try await Task.sleep(for: .milliseconds(200))
        for i in grown.indices {
            biting = true
            withAnimation(.easeOut(duration: 0.15)) { grown[i] = true }
            withAnimation(.easeOut(duration: 0.075)) { squash = true }
            try await Task.sleep(for: .milliseconds(75))
            withAnimation(.easeIn(duration: 0.075)) { squash = false }
            try await Task.sleep(for: .milliseconds(75))
            biting = false
            if i < grown.count - 1 { try await Task.sleep(for: .milliseconds(120)) }
        }
        // Still loading: hold the finished icon with a gentle pulse.
        withAnimation(.easeInOut(duration: 0.8).repeatForever()) { pulse = true }
    }
}
