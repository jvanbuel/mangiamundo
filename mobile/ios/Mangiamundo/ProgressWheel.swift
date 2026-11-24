import SwiftUI

/// A ring filling up while an area's restaurants load, with the step below it.
struct AreaProgressView: View {
    let progress: AreaProgress

    var body: some View {
        VStack(spacing: 10) {
            ProgressRing(fraction: Double(progress.percent) / 100)
                .overlay { Text("\(progress.percent)%").font(Theme.font(.title3, .extraBold)) }
                .frame(width: 84, height: 84)
            Text(progress.text)
                .font(Theme.font(.subheadline, .bold))
                .foregroundStyle(Theme.muted)
        }
        .padding(20)
        .background(Theme.panel, in: RoundedRectangle(cornerRadius: 14))
        .padding(EdgeInsets(top: 2, leading: 2, bottom: 4, trailing: 2))
        .background(Theme.line, in: RoundedRectangle(cornerRadius: 16))
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(progress.text)
        .accessibilityValue("\(progress.percent) percent")
    }
}

/// The accent arc on a track that shows loading: filled to `fraction`, or spinning while it is nil.
struct ProgressRing: View {
    var fraction: Double?
    var lineWidth: CGFloat = 8
    @State private var spinning = false

    var body: some View {
        ZStack {
            Circle().stroke(Theme.line, lineWidth: lineWidth)
            Circle()
                .trim(from: 0, to: fraction ?? 0.3)
                .stroke(Theme.accent, style: StrokeStyle(lineWidth: lineWidth, lineCap: .round))
                .rotationEffect(.degrees(spinning ? 270 : -90))
        }
        .animation(.easeOut(duration: 0.4), value: fraction)
        .onAppear {
            guard fraction == nil else { return }
            withAnimation(.linear(duration: 0.9).repeatForever(autoreverses: false)) { spinning = true }
        }
    }
}
