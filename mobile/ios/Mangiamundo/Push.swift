import UIKit

/// News of food the user hasn't tasted showing up nearby. Unlike celebrations these can't
/// be known in advance, so the server sends them through Apple's push service (APNs). This
/// gets the device token APNs delivers to.
@MainActor
final class DevicePush {
    static let shared = DevicePush()

    /// Development builds get tokens for the APNs sandbox.
    #if DEBUG
    static let sandbox = true
    #else
    static let sandbox = false
    #endif

    private var waiting: [CheckedContinuation<String, Error>] = []

    /// Registers with APNs and returns the device token (hex). Ask for notification
    /// permission first.
    func token() async throws -> String {
        try await withCheckedThrowingContinuation { continuation in
            waiting.append(continuation)
            UIApplication.shared.registerForRemoteNotifications()
        }
    }

    func unregister() {
        UIApplication.shared.unregisterForRemoteNotifications()
    }

    fileprivate func received(_ result: Result<String, Error>) {
        let continuations = waiting
        waiting = []
        for continuation in continuations { continuation.resume(with: result) }
    }

    /// The country in a link such as `/?country=GE`.
    nonisolated static func country(in url: String?) -> String? {
        guard let url, let components = URLComponents(string: "https://x\(url)") else { return nil }
        return components.queryItems?.first { $0.name == "country" }?.value
    }
}

/// Receives the device token from the system.
final class AppDelegate: NSObject, UIApplicationDelegate {
    func application(_ application: UIApplication, didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data) {
        let token = deviceToken.map { String(format: "%02x", $0) }.joined()
        Task { @MainActor in DevicePush.shared.received(.success(token)) }
    }

    func application(_ application: UIApplication, didFailToRegisterForRemoteNotificationsWithError error: Error) {
        Task { @MainActor in DevicePush.shared.received(.failure(error)) }
    }
}
