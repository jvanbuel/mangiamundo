import Foundation

/// The account's recovery code status and the devices signed in to it.
struct Account: Decodable {
    let createdAt: Int
    /// When the current recovery code was made; nil when there is none yet.
    let recoveryCodeCreatedAt: Int?
    /// Most recently used first.
    let devices: [Device]
    /// The public page of the user's progress, like `/p/abcd2345ef`; nil when it's off.
    let publicProfile: String?
    /// The ways to sign in, oldest first; empty for an account that lives on its devices.
    let identities: [Identity]

    /// What a device without an account shows: nothing is on the server until its first change.
    static let none = Account(createdAt: 0, recoveryCodeCreatedAt: nil, devices: [], publicProfile: nil, identities: [])
}

/// A way to sign in: Sign in with Apple, Google or an email address.
struct Identity: Decodable, Hashable {
    /// "apple", "google" or "email".
    let provider: String
    let email: String?
}

/// A public link to share, with its card image and a sentence to go with it.
struct ShareLink: Decodable {
    let url: String
    let imageUrl: String
    let text: String
}

/// A device signed in to the account.
struct Device: Decodable, Identifiable {
    let id: String
    /// What the app called itself; nil for devices from before devices had names.
    let name: String?
    let createdAt: Int
    let lastUsedAt: Int
    /// The device this app runs on.
    let current: Bool
}

/// From `POST /api/session` and the sign-in endpoints: what the device sends from then on.
struct Session: Decodable {
    let token: String
}

/// From `POST /api/account/recovery-code`. Shown once.
struct RecoveryCode: Decodable {
    let code: String
}

/// A code that signs another device in to the account, once, until `expiresAt` (Unix seconds).
struct LinkCode: Decodable, Equatable {
    let code: String
    let expiresAt: Int
}
