import Foundation

/// Timing of the startup loader: the logo globe gets three bites taken out of it.
/// Matches tools/build-logo.mjs (web, Android splash icon) and StartupLoader.kt.
enum StartupLoader {
    /// Whole globe before the first bite.
    static let start: TimeInterval = 0.200
    /// How long one bite takes to grow.
    static let grow: TimeInterval = 0.150
    /// Time between the starts of two bites.
    static let step: TimeInterval = 0.270
    static let bites = 3
    /// Until the last bite is in.
    static let total = start + Double(bites - 1) * step + grow
    /// Fade-out once loading is done.
    static let fade: TimeInterval = 0.250

    /// When bite `index` starts growing.
    static func biteStart(_ index: Int) -> TimeInterval { start + Double(index) * step }

    /// The bite growing `elapsed` seconds after the loader started, if any.
    static func biting(_ elapsed: TimeInterval) -> Int? {
        (0..<bites).first { elapsed >= biteStart($0) && elapsed < biteStart($0) + grow }
    }

    /// How long to wait before fading out once the first data is in: let a growing bite finish.
    static func wait(after elapsed: TimeInterval) -> TimeInterval {
        guard let bite = biting(elapsed) else { return 0 }
        return biteStart(bite) + grow - elapsed
    }
}
