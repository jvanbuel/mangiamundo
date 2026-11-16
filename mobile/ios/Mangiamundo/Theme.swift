import SwiftUI
import UIKit

/// The palette of web/src/app.css, so every client looks the same. Each colour
/// follows the system light/dark setting.
enum Theme {
    static let bg = Color(light: 0xF2F5F8, dark: 0x0D1A28)
    static let panel = Color(light: 0xFFFFFF, dark: 0x132536)
    static let panelRaised = Color(light: 0xEEF3F8, dark: 0x1A3047)
    static let line = Color(light: 0xD5DEE8, dark: 0x28425D)
    static let text = Color(light: 0x14263A, dark: 0xEAF1F8)
    static let muted = Color(light: 0x5A6A7D, dark: 0x93A6BB)
    static let accent = Color(light: 0x2873C2, dark: 0x5BA6EE)
    static let onAccent = Color(light: 0xFFFFFF, dark: 0x07182A)
    static let danger = Color(light: 0xB3261E, dark: 0xFF9D8A)
    static let ocean = Color(light: 0xC4DAEF, dark: 0x0F2133)
    static let land = Color(light: 0xE3E7EB, dark: 0x233A52)
    static let available = Color(light: 0x2873C2, dark: 0x3B8FD9)
    static let discovered = Color(light: 0xC98508, dark: 0xF2B53D)
    static let completed = Color(light: 0x3DA35A, dark: 0x54B96F)
    static let onCompleted = Color(light: 0x0B2A14, dark: 0x06210F)

    /// Nunito, a rounded sans used in heavy weights. Sizes follow the system text
    /// styles, so text still scales with the user's Dynamic Type setting.
    enum Weight: String {
        case regular = "Nunito-Regular"
        case medium = "Nunito-Medium"
        case semibold = "Nunito-SemiBold"
        case bold = "Nunito-Bold"
        case extraBold = "Nunito-ExtraBold"
        case black = "Nunito-Black"
    }

    static func font(_ style: Font.TextStyle, _ weight: Weight = .medium) -> Font {
        let size: CGFloat
        switch style {
        case .largeTitle: size = 34
        case .title: size = 28
        case .title2: size = 22
        case .title3: size = 20
        case .headline: size = 17
        case .subheadline: size = 15
        case .callout: size = 16
        case .footnote: size = 13
        case .caption: size = 12
        case .caption2: size = 11
        default: size = 17
        }
        return .custom(weight.rawValue, size: size, relativeTo: style)
    }

    static func color(for status: Status) -> Color {
        switch status {
        case .available: return available
        case .discovered: return discovered
        case .completed: return completed
        }
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
