import SwiftUI

/// The startup loader: the whole globe from the launch screen gets three bites taken out of it,
/// one after another, until it matches the app icon. Shown until the first data is in.
struct StartupLoaderView: View {
    /// The first data is in (or failed).
    let ready: Bool
    let onFinished: () -> Void

    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var bitten = Array(repeating: false, count: StartupLoader.bites)
    @State private var squashed = false
    @State private var pulsing = false
    @State private var opacity = 1.0
    @State private var startedAt = Date()
    @State private var bites: Task<Void, Never>?
    @State private var leaving = false

    /// Same size as the launch screen image (LaunchGlobe), drawn on a 512 grid like the icon.
    private let size: CGFloat = 180
    private let biteCircles: [(x: CGFloat, y: CGFloat, r: CGFloat)] = [(398, 118, 62), (318, 92, 46), (436, 196, 44)]

    var body: some View {
        ZStack {
            Theme.bg
            ZStack(alignment: .topLeading) {
                Image("LaunchGlobe")
                    .resizable()
                    .frame(width: size, height: size)
                    .scaleEffect(x: squashed ? 0.96 : 1, y: squashed ? 1.03 : 1, anchor: globeCenter)
                ForEach(biteCircles.indices, id: \.self) { i in
                    let bite = biteCircles[i]
                    Circle()
                        .fill(Theme.bg)
                        .frame(width: scaled(2 * bite.r), height: scaled(2 * bite.r))
                        .scaleEffect(bitten[i] ? 1 : 0.001)
                        .position(x: scaled(bite.x), y: scaled(bite.y))
                }
            }
            .frame(width: size, height: size)
            .scaleEffect(pulsing ? 1.03 : 1)
        }
        .ignoresSafeArea()
        .opacity(opacity)
        .accessibilityElement()
        .accessibilityLabel("Mangiamundo is loading")
        .onAppear(perform: play)
        .onChange(of: ready) { if ready { leave() } }
    }

    private var globeCenter: UnitPoint { UnitPoint(x: 256 / 512, y: 262 / 512) }
    private func scaled(_ value: CGFloat) -> CGFloat { value * size / 512 }

    private func play() {
        startedAt = Date()
        if reduceMotion {
            bitten = bitten.map { _ in true }
            if ready { leave() }
            return
        }
        bites = Task { @MainActor in
            for i in 0..<StartupLoader.bites {
                await sleep(until: StartupLoader.biteStart(i))
                if Task.isCancelled { return }
                withAnimation(.spring(duration: StartupLoader.grow, bounce: 0.35)) { bitten[i] = true }
                withAnimation(.easeOut(duration: 0.077)) { squashed = true }
                await sleep(until: StartupLoader.biteStart(i) + 0.077)
                withAnimation(.easeOut(duration: 0.143)) { squashed = false }
            }
            // Still loading: hold the finished icon with a subtle pulse instead of more bites.
            await sleep(until: StartupLoader.total + 0.3)
            if Task.isCancelled { return }
            withAnimation(.easeInOut(duration: 0.8).repeatForever()) { pulsing = true }
        }
        if ready { leave() }
    }

    /// Waits until all bites are in, then fades out.
    private func leave() {
        guard !leaving else { return }
        leaving = true
        Task { @MainActor in
            let wait = reduceMotion ? 0 : StartupLoader.wait(after: Date().timeIntervalSince(startedAt))
            try? await Task.sleep(for: .seconds(wait))
            bites?.cancel() // only the pulse can be left to stop
            withAnimation(.easeOut(duration: reduceMotion ? 0 : StartupLoader.fade)) { opacity = 0 }
            try? await Task.sleep(for: .seconds(reduceMotion ? 0 : StartupLoader.fade))
            onFinished()
        }
    }

    private func sleep(until offset: TimeInterval) async {
        let remaining = offset - Date().timeIntervalSince(startedAt)
        if remaining > 0 { try? await Task.sleep(for: .seconds(remaining)) }
    }
}
