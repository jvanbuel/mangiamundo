import SwiftUI
import UIKit

/// The palette of web/src/app.css, so every client looks the same: blue for the app
/// and for countries still to discover, gold once a country is tasted, and a
/// sparkling diamond once it is completed. Each colour follows the system light/dark
/// setting.
enum Theme {
    static let bg = Color(light: 0xF2F5F8, dark: 0x0D1A28)
    static let panel = Color(light: 0xFFFFFF, dark: 0x132536)
    static let panelRaised = Color(light: 0xEEF3F8, dark: 0x1A3047)
    static let line = Color(light: 0xD5DEE8, dark: 0x28425D)
    static let text = Color(light: 0x14263A, dark: 0xEAF1F8)
    static let muted = Color(light: 0x5A6A7D, dark: 0x93A6BB)
    static let accent = Color(light: 0x2873C2, dark: 0x5BA6EE)
    static let onAccent = Color(light: 0xFFFFFF, dark: 0x07182A)
    /// Accent-coloured text on tinted backgrounds.
    static let accentText = Color(light: 0x1D5A9E, dark: 0x8CC3F5)
    static let danger = Color(light: 0xB3261E, dark: 0xFF9D8A)
    static let ocean = Color(light: 0xDBE5EF, dark: 0x0A1726)
    static let land = Color(light: 0xFBFAF7, dark: 0x22364B)
    static let mapBorder = Color(light: 0xCFD8E2, dark: 0x0A1726)
    static let available = Color(light: 0x2873C2, dark: 0x3B8FD9)
    static let availableEdge = Color(light: 0x1D5A9E, dark: 0x8CC3F5)
    static let discovered = Color(light: 0xE0A100, dark: 0xF2C14E)
    static let discoveredEdge = Color(light: 0x8A6200, dark: 0xF2C14E)
    static let discoveredText = Color(light: 0x7A5600, dark: 0xF2C14E)
    static let onDiscovered = Color(light: 0x3D2A00, dark: 0x2A1A00)
    static let platinum = Color(light: 0xC3C8D0, dark: 0xC9CED6)
    static let platinumEdge = Color(light: 0x5F6773, dark: 0xEEF1F5)
    /// Platinum as a surface: a cool metallic sheen.
    static let platinumMetal = LinearGradient(
        stops: [
            .init(color: Color(hex: 0xF4F6F8), location: 0),
            .init(color: Color(hex: 0xC3C8D0), location: 0.45),
            .init(color: Color(hex: 0xEEF0F3), location: 0.6),
            .init(color: Color(hex: 0xAAB1BB), location: 1),
        ],
        startPoint: .topLeading,
        endPoint: .bottomTrailing
    )
    /// The diamond's base fill on the map.
    static let completed = Color(light: 0xE3F5FE, dark: 0xE3F5FE)
    static let completedEdge = Color(light: 0x2F8FC4, dark: 0x8FD6FF)
    static let completedFacet = Color(light: 0x8FCDEF, dark: 0x8FCDEF)
    static let onCompleted = Color(light: 0x0B4A6B, dark: 0x0B4A6B)
    /// Limited-time events and celebrations: something to act on, so the action colour.
    static let event = Theme.accent
    /// The empty part of a progress bar, and its outline.
    static let track = Color(light: 0xDBE2EA, dark: 0x24405C)
    static let trackEdge = Color(light: 0x8796A8, dark: 0x57708C)

    /// The diamond as a surface: icy, with a hint of prism.
    static let diamond = LinearGradient(
        stops: [
            .init(color: Color(hex: 0xF7FDFF), location: 0),
            .init(color: Color(hex: 0xC9ECFD), location: 0.35),
            .init(color: Color(hex: 0xF1EAFF), location: 0.5),
            .init(color: Color(hex: 0xBDE5FB), location: 0.65),
            .init(color: Color(hex: 0xF0FBFF), location: 1),
        ],
        startPoint: .topLeading,
        endPoint: .bottomTrailing
    )

    /// A full progress bar in icy blue with a bright band.
    static let diamondBar = LinearGradient(
        colors: [completedEdge, Color(hex: 0xBFEAFF), completedEdge],
        startPoint: .leading,
        endPoint: .trailing
    )

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

}

extension Color {
    /// The same colour in light and dark mode.
    init(hex: UInt32) { self.init(light: hex, dark: hex) }

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
