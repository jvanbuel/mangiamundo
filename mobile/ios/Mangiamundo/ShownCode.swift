import SwiftUI
import UIKit

/// A code to read off the screen or copy, in groups of four.
struct ShownCode: View {
    let code: String

    var body: some View {
        HStack(spacing: 12) {
            Text(code)
                .font(.system(.title3, design: .monospaced).weight(.bold))
                .foregroundStyle(Theme.text)
                .textSelection(.enabled)
            Button("Copy") { UIPasteboard.general.string = code }
                .font(Theme.font(.subheadline, .bold))
                .foregroundStyle(Theme.accent)
        }
    }
}
