import SwiftUI
import UIKit

/// Shows the recovery code and lets the user restore another passport onto this phone.
struct BackupSheet: View {
    @Environment(AppModel.self) private var model
    let onDone: () -> Void
    @State private var typed = ""
    @State private var busy = false
    @State private var confirmRotate = false
    @State private var copied = false

    private var typedLength: Int { typed.filter { $0.isLetter || $0.isNumber }.count }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Back up your passport").font(Theme.font(.title2, .black))
            Text("Your progress is tied to this phone. Keep this code somewhere safe to get it back on a new phone or on the web. Anyone with the code can open your passport.")
                .font(Theme.font(.subheadline))
                .foregroundStyle(Theme.muted)

            VStack(alignment: .leading, spacing: 4) {
                Text("Recovery code").font(Theme.font(.footnote, .bold)).foregroundStyle(Theme.muted)
                Text(model.recoveryCode ?? "Loading…")
                    .font(.system(size: 20, weight: .bold, design: .monospaced))
                    .textSelection(.enabled)
                HStack(spacing: 16) {
                    Button(copied ? "Copied" : "Copy", systemImage: "doc.on.doc") {
                        UIPasteboard.general.string = model.recoveryCode
                        copied = true
                    }
                    Button("New code") { confirmRotate = true }
                }
                .font(Theme.font(.subheadline, .extraBold))
                .foregroundStyle(Theme.accent)
                .buttonStyle(.plain)
                .disabled(model.recoveryCode == nil)
                .padding(.top, 2)
            }
            .padding(14)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Theme.panelRaised, in: RoundedRectangle(cornerRadius: 12))

            Divider().overlay(Theme.line).padding(.vertical, 4)

            Text("Restore from another device").font(Theme.font(.headline, .extraBold))
            HStack(spacing: 8) {
                TextField("XXXX-XXXX-XXXX-XXXX", text: $typed)
                    .font(.system(.body, design: .monospaced))
                    .textInputAutocapitalization(.characters)
                    .autocorrectionDisabled()
                    .padding(10)
                    .overlay(RoundedRectangle(cornerRadius: 10).stroke(Theme.line, lineWidth: 2))
                Button(busy ? "Restoring…" : "Restore") {
                    busy = true
                    Task {
                        if await model.restore(code: typed) { onDone() }
                        busy = false
                    }
                }
                .buttonStyle(ChunkyButtonStyle())
                .disabled(busy || typedLength != 16)
                .opacity(busy || typedLength != 16 ? 0.6 : 1)
            }
            if (model.discover?.profile.visits ?? 0) > 0 {
                Text("The check-ins made on this phone are added to the restored passport.")
                    .font(Theme.font(.footnote))
                    .foregroundStyle(Theme.muted)
            }
        }
        .task { await model.loadRecoveryCode() }
        .confirmationDialog("Make a new code? The old code stops working.", isPresented: $confirmRotate, titleVisibility: .visible) {
            Button("New code", role: .destructive) {
                Task { await model.rotateRecoveryCode() }
            }
        }
    }
}

/// A preview of the server-drawn share card, with the system share sheet behind a button.
struct ShareSheet: View {
    @Environment(AppModel.self) private var model
    @Environment(\.colorScheme) private var colorScheme
    @State private var image: UIImage?
    @State private var failed = false

    private var text: String {
        let tasted = model.discover?.world.discovered ?? 0
        let city = model.location?.name ?? "my city"
        if tasted > 0 {
            return "I've tasted \(tasted) \(tasted == 1 ? "country" : "countries") without leaving \(city). Eat the world with Mangiamundo!"
        }
        return "I'm eating my way around the world in \(city) with Mangiamundo."
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Share your map").font(Theme.font(.title2, .black))
            ZStack {
                Theme.panelRaised
                if let image {
                    Image(uiImage: image)
                        .resizable()
                        .scaledToFit()
                        .accessibilityLabel(text)
                } else if failed {
                    Text("Could not draw the card.").foregroundStyle(Theme.muted)
                } else {
                    ProgressView("Drawing your card…")
                }
            }
            .aspectRatio(4 / 5, contentMode: .fit)
            .clipShape(RoundedRectangle(cornerRadius: 14))

            if let image {
                let picture = Image(uiImage: image)
                ShareLink(item: picture, message: Text(text), preview: SharePreview("My Mangiamundo map", image: picture)) {
                    Label("Share", systemImage: "square.and.arrow.up").frame(maxWidth: .infinity)
                }
                .buttonStyle(ChunkyButtonStyle())
            }
        }
        .task(id: colorScheme) {
            image = nil
            failed = false
            if let data = await model.shareCard(dark: colorScheme == .dark) {
                image = UIImage(data: data)
            }
            failed = image == nil
        }
    }
}
