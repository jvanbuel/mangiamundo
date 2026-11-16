package app.mangiamundo.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color

/** The palette of web/src/app.css, so every client looks the same. */
data class AppColors(
    val bg: Color,
    val panel: Color,
    val panelRaised: Color,
    val line: Color,
    val text: Color,
    val muted: Color,
    val accent: Color,
    val onAccent: Color,
    val danger: Color,
    val ocean: Color,
    val land: Color,
    val available: Color,
    val discovered: Color,
    val completed: Color,
    val onCompleted: Color,
)

val LightColors = AppColors(
    bg = Color(0xFFF2F5F8),
    panel = Color(0xFFFFFFFF),
    panelRaised = Color(0xFFEEF3F8),
    line = Color(0xFFD5DEE8),
    text = Color(0xFF14263A),
    muted = Color(0xFF5A6A7D),
    accent = Color(0xFF2873C2),
    onAccent = Color(0xFFFFFFFF),
    danger = Color(0xFFB3261E),
    ocean = Color(0xFFC4DAEF),
    land = Color(0xFFE3E7EB),
    available = Color(0xFF2873C2),
    discovered = Color(0xFFC98508),
    completed = Color(0xFF3DA35A),
    onCompleted = Color(0xFF0B2A14),
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
    danger = Color(0xFFFF9D8A),
    ocean = Color(0xFF0F2133),
    land = Color(0xFF233A52),
    available = Color(0xFF3B8FD9),
    discovered = Color(0xFFF2B53D),
    completed = Color(0xFF54B96F),
    onCompleted = Color(0xFF06210F),
)

val LocalAppColors = staticCompositionLocalOf { LightColors }

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
        MaterialTheme(colorScheme = scheme, content = content)
    }
}
