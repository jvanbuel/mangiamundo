import SwiftUI
import UIKit

/// The palette of web/src/app.css, so every client looks the same. Each colour
/// follows the system light/dark setting.
enum Theme {
    static let bg = Color(light: 0xF4EFE6, dark: 0x0B1220)
    static let panel = Color(light: 0xFFFDF9, dark: 0x141F33)
    static let panelRaised = Color(light: 0xF3ECE0, dark: 0x1B2942)
    static let line = Color(light: 0xDFD5C4, dark: 0x2A3A58)
    static let text = Color(light: 0x1C2333, dark: 0xEEF2F8)
    static let muted = Color(light: 0x626B7F, dark: 0x96A5BE)
    static let accent = Color(light: 0xC5481F, dark: 0xFF8A5C)
    static let onAccent = Color(light: 0xFFFFFF, dark: 0x1B0F08)
    static let danger = Color(light: 0xB3261E, dark: 0xFF9D8A)
    static let ocean = Color(light: 0xDCE8F1, dark: 0x0F1A2E)
    static let land = Color(light: 0xC3CBD6, dark: 0x223150)
    static let available = Color(light: 0xE8694A, dark: 0xE8694A)
    static let discovered = Color(light: 0xEFA51C, dark: 0xF4B942)
    static let completed = Color(light: 0x27A872, dark: 0x3ECF8E)
    static let onCompleted = Color(light: 0xFFFFFF, dark: 0x06281A)

    static func color(for status: Status) -> Color {
        switch status {
        case .available: return available
        case .discovered: return discovered
        case .completed: return completed
        }
    }

    static func display(_ size: CGFloat, weight: Font.Weight = .semibold) -> Font {
        .system(size: size, weight: weight, design: .serif)
    }
}

extension Color {
    init(light: UInt32, dark: UInt32) {
        self.init(uiColor: UIColor { traits in
            let hex = traits.userInterfaceStyle == .dark ? dark : light
            return UIColor(
                red: CGFloat((hex >> 16) & 0xFF) / 255,
                green: CGFloat((hex >> 8) & 0xFF) / 255,
                blue: CGFloat(hex & 0xFF) / 255,
                alpha: 1
            )
        })
    }
}
