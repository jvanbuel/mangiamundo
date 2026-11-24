import SwiftUI

struct ToastStack: View {
    let toasts: [Toast]

    var body: some View {
        VStack(spacing: 8) {
            ForEach(toasts) { toast in
                Text(toast.text)
                    .font(Theme.font(.subheadline, .bold))
                    .padding(.horizontal, 14)
                    .padding(.vertical, 12)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(Theme.panelRaised, in: RoundedRectangle(cornerRadius: 14))
                    .overlay(RoundedRectangle(cornerRadius: 14).stroke(Theme.accent))
                    .shadow(color: .black.opacity(0.25), radius: 12, y: 6)
                    .transition(.move(edge: .top).combined(with: .opacity))
            }
        }
        .padding(.horizontal, 16)
        .padding(.top, 8)
        .animation(.spring(duration: 0.3), value: toasts)
        .allowsHitTesting(false)
    }
}
