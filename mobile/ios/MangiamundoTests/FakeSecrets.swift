import Security
import XCTest
@testable import Mangiamundo

/// Stands in for the Keychain; `failing` makes it refuse, as a locked phone does.
final class FakeSecrets: SecretStore, @unchecked Sendable {
    var value: String?
    var failing = false

    func read() throws -> String? {
        if failing { throw KeychainError(status: errSecInteractionNotAllowed) }
        return value
    }

    func write(_ value: String?) throws {
        if failing { throw KeychainError(status: errSecInteractionNotAllowed) }
        self.value = value
    }
}
