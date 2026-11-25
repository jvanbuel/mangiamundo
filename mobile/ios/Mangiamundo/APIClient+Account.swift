import Foundation

/// The account: recovery and link codes, signing in and out, devices.
extension APIClient {
    /// Whether this device has an account yet; it gets one with its first check-in or other change.
    var hasAccount: Bool { tokens.token != nil }

    /// The account's recovery code status and the devices signed in to it. A device without
    /// an account has an empty one, with no devices: nothing is on the server until its
    /// first change.
    func account() async throws -> Account {
        guard let data = try await mine("/api/account") else { return .none }
        return try decode(data)
    }

    /// A new recovery code, shown once. The previous one stops working.
    func newRecoveryCode() async throws -> String {
        let made: RecoveryCode = try decode(await write("POST", "/api/account/recovery-code"))
        return made.code
    }

    /// A code that signs another device in to this account, once, within 10 minutes.
    func newLinkCode() async throws -> LinkCode {
        try decode(await write("POST", "/api/account/link-code"))
    }

    /// Signs this device in to the account of a recovery or link code. What this device did
    /// before moves into that account. An unknown, used or expired code is a 404.
    func restore(code: String) async throws {
        try await switchAccount("/api/session/restore", ["code": code])
    }

    /// Emails a 6-digit sign-in code.
    func sendEmailCode(_ email: String) async throws {
        let data = try JSONSerialization.data(withJSONObject: ["email": email])
        _ = try await request("POST", "/api/auth/email/code", body: data)
    }

    /// Signs in with an emailed code. A wrong, used or expired code is a 404.
    func signIn(email: String, code: String) async throws {
        try await switchAccount("/api/auth/email", ["email": email, "code": code])
    }

    /// Signs in with an ID token from Sign in with Apple.
    func signInWithApple(idToken: String) async throws {
        try await switchAccount("/api/auth/apple", ["id_token": idToken])
    }

    /// Signs in with an ID token from Google's sign-in.
    func signInWithGoogle(idToken: String) async throws {
        try await switchAccount("/api/auth/google", ["id_token": idToken])
    }

    /// Signs this device out of its account; it starts afresh, without one.
    func signOut() async throws {
        if let me = try await account().devices.first(where: \.current) {
            try await removeDevice(id: me.id)
        }
        tokens.token = nil
    }

    /// Sends the device's token along to an endpoint that answers with another account's
    /// session (what this device held merges into it), and keeps that session.
    private func switchAccount(_ path: String, _ fields: [String: String]) async throws {
        var body = fields
        body["device"] = deviceName
        let data = try JSONSerialization.data(withJSONObject: body)
        let current = tokens.token
        let session: Session
        do {
            session = try decode(await request("POST", path, body: data, token: current))
        } catch let error as APIError where error.status == 401 && current != nil {
            // This device was signed out elsewhere: drop that token and go on without it.
            if tokens.token == current { tokens.token = nil }
            session = try decode(await request("POST", path, body: data))
        }
        tokens.token = session.token
    }

    /// Signs another device out of this account.
    func removeDevice(id: String) async throws {
        try await writeMine("DELETE", "/api/account/devices/\(encode(id))")
    }

    /// Deletes the account and everything in it. This device starts over without one.
    func deleteAccount() async throws {
        try await writeMine("DELETE", "/api/account")
        tokens.token = nil
    }
}
