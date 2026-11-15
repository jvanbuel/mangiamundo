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
    bg = Color(0xFFF4EFE6),
    panel = Color(0xFFFFFDF9),
    panelRaised = Color(0xFFF3ECE0),
    line = Color(0xFFDFD5C4),
    text = Color(0xFF1C2333),
    muted = Color(0xFF626B7F),
    accent = Color(0xFFC5481F),
    onAccent = Color(0xFFFFFFFF),
    danger = Color(0xFFB3261E),
    ocean = Color(0xFFDCE8F1),
    land = Color(0xFFC3CBD6),
    available = Color(0xFFE8694A),
    discovered = Color(0xFFEFA51C),
    completed = Color(0xFF27A872),
    onCompleted = Color(0xFFFFFFFF),
)

val DarkColors = AppColors(
    bg = Color(0xFF0B1220),
    panel = Color(0xFF141F33),
    panelRaised = Color(0xFF1B2942),
    line = Color(0xFF2A3A58),
    text = Color(0xFFEEF2F8),
    muted = Color(0xFF96A5BE),
    accent = Color(0xFFFF8A5C),
    onAccent = Color(0xFF1B0F08),
    danger = Color(0xFFFF9D8A),
    ocean = Color(0xFF0F1A2E),
    land = Color(0xFF223150),
    available = Color(0xFFE8694A),
    discovered = Color(0xFFF4B942),
    completed = Color(0xFF3ECF8E),
    onCompleted = Color(0xFF06281A),
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
