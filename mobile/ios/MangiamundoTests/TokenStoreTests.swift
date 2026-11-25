import XCTest
@testable import Mangiamundo

/// Where the session token is kept, and how one from before the Keychain gets there.
final class TokenStoreTests: XCTestCase {
    private var suite: String!
    private var defaults: UserDefaults!
    private var secrets: FakeSecrets!

    override func setUp() {
        suite = "mangiamundo.tests.\(UUID().uuidString)"
        defaults = UserDefaults(suiteName: suite)
        secrets = FakeSecrets()
    }

    override func tearDown() {
        defaults.removePersistentDomain(forName: suite)
    }

    func testATokenFromUserDefaultsMovesToTheKeychainOnce() {
        defaults.set("old-token", forKey: TokenStore.legacyKey)

        let store = TokenStore(secrets: secrets, legacy: defaults)

        XCTAssertEqual(store.token, "old-token")
        XCTAssertEqual(secrets.value, "old-token")
        XCTAssertNil(defaults.string(forKey: TokenStore.legacyKey))
        // The next launch finds nothing left to move, and the token where it belongs.
        secrets.value = "newer-token"
        XCTAssertEqual(TokenStore(secrets: secrets, legacy: defaults).token, "newer-token")
    }

    func testTokensAreKeptInTheKeychainOnly() {
        let store = TokenStore(secrets: secrets, legacy: defaults)
        XCTAssertNil(store.token)

        store.token = "token"
        XCTAssertEqual(secrets.value, "token")
        XCTAssertNil(defaults.string(forKey: TokenStore.legacyKey))
        XCTAssertEqual(store.token, "token")

        store.token = nil
        XCTAssertNil(secrets.value)
        XCTAssertNil(store.token)
    }

    func testATokenWaitsInUserDefaultsWhileTheKeychainRefuses() {
        defaults.set("old-token", forKey: TokenStore.legacyKey)
        secrets.failing = true

        let store = TokenStore(secrets: secrets, legacy: defaults)

        // Not moved, and not lost.
        XCTAssertEqual(store.token, "old-token")
        XCTAssertEqual(defaults.string(forKey: TokenStore.legacyKey), "old-token")
        store.token = "new-token"
        XCTAssertEqual(store.token, "new-token")
        XCTAssertNil(secrets.value)

        // Once the Keychain takes it, at the next launch, it moves.
        secrets.failing = false
        let later = TokenStore(secrets: secrets, legacy: defaults)
        XCTAssertEqual(later.token, "new-token")
        XCTAssertEqual(secrets.value, "new-token")
        XCTAssertNil(defaults.string(forKey: TokenStore.legacyKey))
    }

    func testSigningOutHoldsWhileTheKeychainRefuses() {
        secrets.value = "token"
        let store = TokenStore(secrets: secrets, legacy: defaults)
        secrets.failing = true

        store.token = nil

        // The Keychain still has the token; the device is signed out all the same.
        XCTAssertNil(store.token)
        secrets.failing = false
        XCTAssertNil(store.token)
        XCTAssertNil(TokenStore(secrets: secrets, legacy: defaults).token)
        XCTAssertNil(secrets.value)
        XCTAssertNil(defaults.string(forKey: TokenStore.legacyKey))
    }
}
