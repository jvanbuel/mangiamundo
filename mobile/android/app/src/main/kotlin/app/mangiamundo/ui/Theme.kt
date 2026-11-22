package app.mangiamundo.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.material3.Typography
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import app.mangiamundo.R

/**
 * The palette of web/src/app.css, so every client looks the same: blue for the app and
 * for countries still to discover, gold once a country is tasted, and a sparkling
 * diamond once it is completed.
 */
data class AppColors(
    val bg: Color,
    val panel: Color,
    val panelRaised: Color,
    val line: Color,
    val text: Color,
    val muted: Color,
    val accent: Color,
    val onAccent: Color,
    /** Accent-coloured text on tinted backgrounds. */
    val accentText: Color,
    val danger: Color,
    val ocean: Color,
    val land: Color,
    val mapBorder: Color,
    val available: Color,
    val availableEdge: Color,
    val discovered: Color,
    val discoveredEdge: Color,
    val discoveredText: Color,
    val onDiscovered: Color,
    /** The diamond's base fill on the map. */
    val completed: Color,
    val completedEdge: Color,
    val completedFacet: Color,
    val onCompleted: Color,
    /** Limited-time events and celebrations: something to act on, so the action colour. */
    val event: Color,
    /** The empty part of a progress bar, and its outline. */
    val track: Color,
    val trackEdge: Color,
) {
    /** The diamond as a surface: icy, with a hint of prism. */
    val diamond: Brush
        get() = Brush.linearGradient(
            0f to Color(0xFFF7FDFF),
            0.35f to Color(0xFFC9ECFD),
            0.5f to Color(0xFFF1EAFF),
            0.65f to Color(0xFFBDE5FB),
            1f to Color(0xFFF0FBFF),
        )

    /** A full progress bar in icy blue with a bright band. */
    val diamondBar: Brush
        get() = Brush.horizontalGradient(listOf(completedEdge, Color(0xFFBFEAFF), completedEdge))
}

val LightColors = AppColors(
    bg = Color(0xFFF2F5F8),
    panel = Color(0xFFFFFFFF),
    panelRaised = Color(0xFFEEF3F8),
    line = Color(0xFFD5DEE8),
    text = Color(0xFF14263A),
    muted = Color(0xFF5A6A7D),
    accent = Color(0xFF2873C2),
    onAccent = Color(0xFFFFFFFF),
    accentText = Color(0xFF1D5A9E),
    danger = Color(0xFFB3261E),
    ocean = Color(0xFFDBE5EF),
    land = Color(0xFFFBFAF7),
    mapBorder = Color(0xFFCFD8E2),
    available = Color(0xFF2873C2),
    availableEdge = Color(0xFF1D5A9E),
    discovered = Color(0xFFE0A100),
    discoveredEdge = Color(0xFF8A6200),
    discoveredText = Color(0xFF7A5600),
    onDiscovered = Color(0xFF3D2A00),
    completed = Color(0xFFE3F5FE),
    completedEdge = Color(0xFF2F8FC4),
    completedFacet = Color(0xFF8FCDEF),
    onCompleted = Color(0xFF0B4A6B),
    event = Color(0xFF2873C2),
    track = Color(0xFFDBE2EA),
    trackEdge = Color(0xFF8796A8),
)

val DarkColors = AppColors(
    bg = Color(0xFF0D1A28),
    panel = Color(0xFF132536),
    panelRaised = Color(0xFF1A3047),
    line = Color(0xFF28425D),
    text = Color(0xFFEAF1F8),
    muted = Color(0xFF93A6BB),
    accent = Color(0xFF5BA6EE),
    onAccent = Color(0xFF07182A),
    accentText = Color(0xFF8CC3F5),
    danger = Color(0xFFFF9D8A),
    ocean = Color(0xFF0A1726),
    land = Color(0xFF22364B),
    mapBorder = Color(0xFF0A1726),
    available = Color(0xFF3B8FD9),
    availableEdge = Color(0xFF8CC3F5),
    discovered = Color(0xFFF2C14E),
    discoveredEdge = Color(0xFFF2C14E),
    discoveredText = Color(0xFFF2C14E),
    onDiscovered = Color(0xFF2A1A00),
    completed = Color(0xFFE3F5FE),
    completedEdge = Color(0xFF8FD6FF),
    completedFacet = Color(0xFF8FCDEF),
    onCompleted = Color(0xFF0B4A6B),
    event = Color(0xFF5BA6EE),
    track = Color(0xFF24405C),
    trackEdge = Color(0xFF57708C),
)

val LocalAppColors = staticCompositionLocalOf { LightColors }

/** Nunito: a rounded sans, used in heavy weights. Shipped in res/font (SIL Open Font License). */
val Nunito = FontFamily(
    Font(R.font.nunito_regular, FontWeight.Normal),
    Font(R.font.nunito_medium, FontWeight.Medium),
    Font(R.font.nunito_semibold, FontWeight.SemiBold),
    Font(R.font.nunito_bold, FontWeight.Bold),
    Font(R.font.nunito_extrabold, FontWeight.ExtraBold),
    Font(R.font.nunito_black, FontWeight.Black),
)

/** The Material type scale in Nunito, one weight step heavier than Material's defaults. */
private val AppTypography = Typography().run {
    fun TextStyle.nunito(weight: FontWeight) = copy(fontFamily = Nunito, fontWeight = weight)
    copy(
        displayLarge = displayLarge.nunito(FontWeight.ExtraBold),
        displayMedium = displayMedium.nunito(FontWeight.ExtraBold),
        displaySmall = displaySmall.nunito(FontWeight.ExtraBold),
        headlineLarge = headlineLarge.nunito(FontWeight.Black),
        headlineMedium = headlineMedium.nunito(FontWeight.Black),
        headlineSmall = headlineSmall.nunito(FontWeight.Black),
        titleLarge = titleLarge.nunito(FontWeight.Black),
        titleMedium = titleMedium.nunito(FontWeight.Bold),
        titleSmall = titleSmall.nunito(FontWeight.Bold),
        bodyLarge = bodyLarge.nunito(FontWeight.Medium),
        bodyMedium = bodyMedium.nunito(FontWeight.Medium),
        bodySmall = bodySmall.nunito(FontWeight.Medium),
        labelLarge = labelLarge.nunito(FontWeight.ExtraBold),
        labelMedium = labelMedium.nunito(FontWeight.Bold),
        labelSmall = labelSmall.nunito(FontWeight.Bold),
    )
}

/** Follows the system light/dark setting. */
@Composable
fun MangiamundoTheme(content: @Composable () -> Unit) {
    val dark = isSystemInDarkTheme()
    val colors = if (dark) DarkColors else LightColors
    val scheme = (if (dark) darkColorScheme() else lightColorScheme()).copy(
        primary = colors.accent,
        onPrimary = colors.onAccent,
        secondaryContainer = colors.accent,
        onSecondaryContainer = colors.onAccent,
        background = colors.bg,
        onBackground = colors.text,
        surface = colors.panel,
        onSurface = colors.text,
        surfaceVariant = colors.panelRaised,
        onSurfaceVariant = colors.muted,
        surfaceContainerLow = colors.panel,
        surfaceContainer = colors.panelRaised,
        surfaceContainerHigh = colors.panelRaised,
        outline = colors.line,
        outlineVariant = colors.line,
        error = colors.danger,
    )
    CompositionLocalProvider(LocalAppColors provides colors) {
        MaterialTheme(colorScheme = scheme, typography = AppTypography, content = content)
    }
}
