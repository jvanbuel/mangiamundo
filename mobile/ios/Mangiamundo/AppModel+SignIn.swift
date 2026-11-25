import Foundation
import GoogleSignIn
import UIKit

/// Signing in with an emailed code, Apple or Google.
extension AppModel {
    func sendEmailCode(_ email: String) {
        accountAction {
            let address = email.trimmingCharacters(in: .whitespaces)
            try await self.api.sendEmailCode(address)
            self.codeSentTo = address
        }
    }

    func forgetEmailCode() {
        codeSentTo = nil
    }

    func signIn(code: String) {
        guard let email = codeSentTo else { return }
        accountAction {
            do {
                try await self.api.signIn(email: email, code: code)
            } catch let error as APIError where error.status == 404 {
                self.toast("Wrong code. Check it, or ask for a new one.")
                return
            }
            self.codeSentTo = nil
            await self.signedIn()
        }
    }

    func signInWithApple(idToken: String) {
        accountAction {
            try await self.api.signInWithApple(idToken: idToken)
            await self.signedIn()
        }
    }

    /// Opens Google's sign-in sheet and signs in with the ID token it hands back.
    func signInWithGoogle() {
        accountAction {
            let window = UIApplication.shared.connectedScenes.compactMap { ($0 as? UIWindowScene)?.keyWindow }.first
            guard var top = window?.rootViewController else { return }
            while let presented = top.presentedViewController { top = presented }
            let result: GIDSignInResult
            do {
                result = try await GIDSignIn.sharedInstance.signIn(withPresenting: top)
            } catch let error as GIDSignInError where error.code == .canceled {
                return
            }
            guard let token = result.user.idToken?.tokenString else {
                self.toast("Google did not hand back a sign-in token. Try again.")
                return
            }
            try await self.api.signInWithGoogle(idToken: token)
            await self.signedIn()
        }
    }

    private func signedIn() async {
        recoveryCode = nil
        linkCode = nil
        toast("Signed in. Your progress now follows you to every device you sign in on.")
        await accountChanged()
    }
}
