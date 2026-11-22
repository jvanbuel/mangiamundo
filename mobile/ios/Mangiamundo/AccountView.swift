import AuthenticationServices
import SwiftUI
import UIKit

/// Recovery code, link code, signed-in devices and account deletion, in the settings sheet.
struct AccountSection: View {
    @Environment(AppModel.self) private var model
    @State private var entered = ""
    @State private var confirmingNewCode = false
    @State private var confirmingDelete = false
    @State private var confirmingSignOut = false
    @State private var email = ""
    @State private var code = ""
    @Environment(\.colorScheme) private var colorScheme
    @State private var now = Date()

    private let tick = Timer.publish(every: 5, on: .main, in: .common).autoconnect()

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Divider().overlay(Theme.line).padding(.vertical, 8)
            signIn

            heading("Your progress")
            if model.account?.identities.isEmpty == false {
                note("A recovery code also gets it back, without signing in.")
            } else {
                note("It lives on this device. Keep a recovery code to get it back after reinstalling the app or on a new phone.")
            }
            if let code = model.recoveryCode {
                ShownCode(code: code)
                note("Shown only now. Anyone with this code can use your account.")
            } else {
                let made = model.account?.recoveryCodeCreatedAt
                Button(made == nil ? "Make a recovery code" : "Make a new recovery code") {
                    if made == nil { model.makeRecoveryCode() } else { confirmingNewCode = true }
                }
                .buttonStyle(ChunkyButtonStyle())
                .disabled(model.accountBusy)
                if let made { note("You made one \(ago(made)).") }
            }

            if let url = model.publicProfileURL {
                heading("Public link")
                note("Anyone with your link can see your map, level and badges. Turning it off breaks the link; sharing again makes a new one.")
                HStack(spacing: 8) {
                    Link(destination: url) { Label("Open", systemImage: "safari") }
                        .buttonStyle(ChunkyButtonStyle(face: Theme.panelRaised, content: Theme.text, small: true))
                    Button("Turn off") { model.unshareProgress() }
                        .buttonStyle(ChunkyButtonStyle(face: Theme.panelRaised, content: Theme.text, small: true))
                        .disabled(model.accountBusy || model.sharing)
                }
            }

            heading("Use it on another device")
            if let link = model.linkCode {
                ShownCode(code: link.code)
                let minutes = max(0, (link.expiresAt - Int(now.timeIntervalSince1970) + 59) / 60)
                note("Enter it there under Settings within \(minutes) \(minutes == 1 ? "minute" : "minutes"). It works once.")
            } else {
                Button("Show a link code") { model.makeLinkCode() }
                    .buttonStyle(ChunkyButtonStyle())
                    .disabled(model.accountBusy)
            }

            heading("Have a recovery or link code?")
            HStack {
                TextField("XXXX-XXXX", text: $entered)
                    .textFieldStyle(.roundedBorder)
                    .textInputAutocapitalization(.characters)
                    .autocorrectionDisabled()
                    .submitLabel(.done)
                    .onSubmit(restore)
                Button("Use", action: restore)
                    .buttonStyle(ChunkyButtonStyle())
                    .disabled(model.accountBusy || entered.trimmingCharacters(in: .whitespaces).isEmpty)
            }
            note("What you did on this device so far is added to that account.")

            if let error = model.accountError {
                note(error)
            } else if let devices = model.account?.devices, devices.count > 1 {
                heading("Signed-in devices")
                ForEach(devices) { device in
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(device.name ?? "Unnamed device").font(Theme.font(.subheadline, .bold))
                            note(device.current ? "This device" : "Used \(ago(device.lastUsedAt))")
                        }
                        Spacer()
                        if !device.current {
                            Button("Sign out") { model.signOut(deviceID: device.id) }
                                .font(Theme.font(.subheadline, .bold))
                                .foregroundStyle(Theme.accent)
                                .disabled(model.accountBusy)
                        }
                    }
                }
            }

            Button("Delete my account") { confirmingDelete = true }
                .font(Theme.font(.subheadline))
                .foregroundStyle(Theme.muted)
                .underline()
                .padding(.top, 8)
                .disabled(model.accountBusy)
        }
        .buttonStyle(.plain)
        .task { await model.loadAccount() }
        .onReceive(tick) { date in
            now = date
            if let link = model.linkCode, link.expiresAt <= Int(date.timeIntervalSince1970) { model.clearLinkCode() }
        }
        .confirmationDialog("Make a new recovery code?", isPresented: $confirmingNewCode, titleVisibility: .visible) {
            Button("Make a new one") { model.makeRecoveryCode() }
        } message: {
            Text("The old one stops working.")
        }
        .confirmationDialog("Sign out of this device?", isPresented: $confirmingSignOut, titleVisibility: .visible) {
            Button("Sign out") { model.signOut() }
        } message: {
            Text("Your progress stays in your account; this device starts afresh.")
        }
        .confirmationDialog("Delete your account?", isPresented: $confirmingDelete, titleVisibility: .visible) {
            Button("Delete", role: .destructive) { model.deleteAccount() }
        } message: {
            Text("Every check-in, badge and added place is gone for good, on all your devices.")
        }
    }

    /// Signing in is optional: with Apple, or with a code sent by email.
    @ViewBuilder private var signIn: some View {
        heading("Sign in")
        if let identities = model.account?.identities, !identities.isEmpty {
            note("Signed in with \(identities.map(describe).joined(separator: ", ")).")
            Button("Sign out of this device") { confirmingSignOut = true }
                .buttonStyle(ChunkyButtonStyle())
                .disabled(model.accountBusy)
        } else {
            note("Optional. Sign in to keep your progress on all your devices; without it, it stays on this one.")
            SignInWithAppleButton(.signIn) { request in
                request.requestedScopes = [.email]
            } onCompletion: { result in
                switch result {
                case .success(let authorization):
                    guard let credential = authorization.credential as? ASAuthorizationAppleIDCredential,
                          let data = credential.identityToken,
                          let token = String(data: data, encoding: .utf8) else { return }
                    model.signInWithApple(idToken: token)
                case .failure(let error):
                    if (error as? ASAuthorizationError)?.code != .canceled {
                        model.toast("Sign in with Apple did not work: \(error.localizedDescription)")
                    }
                }
            }
            .signInWithAppleButtonStyle(colorScheme == .dark ? .white : .black)
            .frame(height: 44)
            .disabled(model.accountBusy)

            if let sentTo = model.codeSentTo {
                note("Enter the code we sent to \(sentTo).")
                HStack {
                    TextField("123456", text: $code)
                        .textFieldStyle(.roundedBorder)
                        .keyboardType(.numberPad)
                        .textContentType(.oneTimeCode)
                    Button("Sign in") { model.signIn(code: code); code = "" }
                        .buttonStyle(ChunkyButtonStyle())
                        .disabled(model.accountBusy || code.trimmingCharacters(in: .whitespaces).isEmpty)
                }
                Button("Use another address") { model.forgetEmailCode() }
                    .font(Theme.font(.caption, .bold))
                    .foregroundStyle(Theme.accent)
            } else {
                note("Or get a code by email.")
                HStack {
                    TextField("you@example.com", text: $email)
                        .textFieldStyle(.roundedBorder)
                        .keyboardType(.emailAddress)
                        .textContentType(.emailAddress)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .submitLabel(.send)
                        .onSubmit { model.sendEmailCode(email) }
                    Button("Send code") { model.sendEmailCode(email) }
                        .buttonStyle(ChunkyButtonStyle())
                        .disabled(model.accountBusy || email.trimmingCharacters(in: .whitespaces).isEmpty)
                }
            }
        }
    }

    private func describe(_ identity: Identity) -> String {
        let name = ["apple": "Apple", "google": "Google", "email": "email"][identity.provider] ?? identity.provider
        return identity.email.map { "\(name) (\($0))" } ?? name
    }

    private func restore() {
        guard !entered.trimmingCharacters(in: .whitespaces).isEmpty else { return }
        model.restore(code: entered) { entered = "" }
    }

    private func heading(_ text: String) -> some View {
        Text(text)
            .font(Theme.font(.subheadline, .extraBold))
            .foregroundStyle(Theme.text)
            .padding(.top, 8)
    }

    private func note(_ text: String) -> some View {
        Text(text)
            .font(Theme.font(.caption))
            .foregroundStyle(Theme.muted)
            .fixedSize(horizontal: false, vertical: true)
    }

    private func ago(_ at: Int) -> String {
        let days = (Int(now.timeIntervalSince1970) - at) / 86_400
        switch days {
        case ..<1: return "today"
        case 1: return "yesterday"
        case ..<60: return "\(days) days ago"
        default: return Date(timeIntervalSince1970: TimeInterval(at)).formatted(date: .abbreviated, time: .omitted)
        }
    }
}

/// A code to read off the screen or copy, in groups of four.
private struct ShownCode: View {
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
