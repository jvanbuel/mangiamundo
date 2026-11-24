import Foundation
import Security

/// Somewhere to keep one secret string.
protocol SecretStore: Sendable {
    /// The secret, or nil when there is none.
    func read() throws -> String?
    /// Keeps `value`, replacing what was there; nil removes it.
    func write(_ value: String?) throws
}

struct KeychainError: Error, Equatable {
    let status: OSStatus
}

/// One password item in the app's Keychain.
struct KeychainItem: SecretStore {
    let service: String
    let account: String

    private var query: [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]
    }

    func read() throws -> String? {
        var search = query
        search[kSecReturnData as String] = true
        search[kSecMatchLimit as String] = kSecMatchLimitOne
        var found: CFTypeRef?
        let status = SecItemCopyMatching(search as CFDictionary, &found)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess, let data = found as? Data else { throw KeychainError(status: status) }
        return String(data: data, encoding: .utf8)
    }

    func write(_ value: String?) throws {
        guard let value else {
            let status = SecItemDelete(query as CFDictionary)
            guard status == errSecSuccess || status == errSecItemNotFound else { throw KeychainError(status: status) }
            return
        }
        let data = Data(value.utf8)
        var status = SecItemUpdate(query as CFDictionary, [kSecValueData as String: data] as CFDictionary)
        if status == errSecItemNotFound {
            var item = query
            item[kSecValueData as String] = data
            // Readable while the phone is locked, once it was unlocked since it started: the
            // app also asks the server things when it isn't in front.
            item[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlock
            status = SecItemAdd(item as CFDictionary, nil)
        }
        guard status == errSecSuccess else { throw KeychainError(status: status) }
    }
}

/// Where this device's session token is kept. The token is the account, so it lives in the
/// Keychain, not in UserDefaults, where it used to be: a token found there is moved once
/// and removed.
///
/// When the Keychain refuses (it can, before the phone's first unlock after a restart) the
/// token waits in UserDefaults instead of getting lost, and is moved the next time one is
/// stored or the app starts.
///
/// Unlike UserDefaults, the Keychain outlives the app: deleting and reinstalling it finds
/// the account again.
struct TokenStore: @unchecked Sendable {
    /// Where the token was kept before, and where one waits that the Keychain refused. An
    /// empty string waiting there means: signed out, whatever the Keychain still holds.
    static let legacyKey = "mangiamundo.token"

    private let secrets: any SecretStore
    // UserDefaults is safe to use from any thread.
    private let legacy: UserDefaults

    init(
        secrets: any SecretStore = KeychainItem(service: "app.mangiamundo.session", account: "token"),
        legacy: UserDefaults = .standard
    ) {
        self.secrets = secrets
        self.legacy = legacy
        if let waiting = legacy.string(forKey: Self.legacyKey) {
            store(waiting.isEmpty ? nil : waiting)
        }
    }

    var token: String? {
        get {
            // What waits in UserDefaults is newer than what the Keychain holds.
            if let waiting = legacy.string(forKey: Self.legacyKey) {
                return waiting.isEmpty ? nil : waiting
            }
            return try? secrets.read()
        }
        nonmutating set { store(newValue) }
    }

    private func store(_ token: String?) {
        do {
            try secrets.write(token)
            legacy.removeObject(forKey: Self.legacyKey)
        } catch {
            legacy.set(token ?? "", forKey: Self.legacyKey)
        }
    }
}
