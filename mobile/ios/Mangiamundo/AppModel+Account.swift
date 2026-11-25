import Foundation

/// The account: recovery and link codes, devices, signing out and deleting it.
extension AppModel {
    func loadAccount() async {
        do {
            account = try await api.account()
            accountError = nil
        } catch {
            accountError = error.localizedDescription
        }
    }

    /// Runs one account action at a time, reporting failures in a toast.
    func accountAction(_ work: @escaping @MainActor () async throws -> Void) {
        guard !accountBusy else { return }
        accountBusy = true
        Task {
            do {
                try await work()
            } catch {
                toast("That did not work: \(error.localizedDescription)")
            }
            accountBusy = false
        }
    }

    func makeRecoveryCode() {
        accountAction {
            self.recoveryCode = try await self.api.newRecoveryCode()
            self.account = try await self.api.account()
        }
    }

    func makeLinkCode() {
        accountAction { self.linkCode = try await self.api.newLinkCode() }
    }

    func clearLinkCode() {
        linkCode = nil
    }

    /// Signs in to the account of a recovery or link code; what this device did moves into it.
    func restore(code: String, onRestored: @escaping @MainActor () -> Void) {
        accountAction {
            do {
                try await self.api.restore(code: code)
            } catch let error as APIError where error.status == 404 {
                self.toast("Code not recognised. A link code works once, for 10 minutes.")
                return
            }
            self.recoveryCode = nil
            self.linkCode = nil
            onRestored()
            self.toast("Welcome back. Your progress is on this device now.")
            await self.accountChanged()
        }
    }

    func signOut(deviceID: String) {
        accountAction {
            try await self.api.removeDevice(id: deviceID)
            self.account = try await self.api.account()
        }
    }

    func signOut() {
        accountAction {
            try await self.api.signOut()
            self.recoveryCode = nil
            self.linkCode = nil
            self.toast("Signed out. Sign in again to get your progress back here.")
            await self.accountChanged()
        }
    }

    func deleteAccount() {
        accountAction {
            try await self.api.deleteAccount()
            self.recoveryCode = nil
            self.linkCode = nil
            self.toast("Account deleted. You start fresh on this device.")
            await self.accountChanged()
        }
    }

    /// The server no longer knows this device: it was signed out from another device, or its
    /// account is gone. Says so, and shows everything again as for someone without an
    /// account, so nothing of the old account stays on screen as if it were still here. A
    /// new account only starts with the next check-in or other change.
    func signedOut() async {
        toast("You were signed out. Sign in or use a recovery code to get your progress back.")
        recoveryCode = nil
        linkCode = nil
        await accountChanged()
    }

    /// After this device switches account: drop the previous account's visits, then load this one's.
    func accountChanged() async {
        accountGeneration += 1
        detailTask?.cancel()
        detail = nil
        // Cancels the places loader, clears its places and loads the open country again.
        select(selected)
        refresh()
        // A failure here only shows in the account section: the switch itself worked.
        await loadAccount()
    }
}
