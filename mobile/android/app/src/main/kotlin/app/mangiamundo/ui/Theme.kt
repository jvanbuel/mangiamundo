package app.mangiamundo.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.material3.Typography
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import app.mangiamundo.R

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
    /** Limited-time events and celebrations. */
    val event: Color,
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
    event = Color(0xFFC2417A),
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
    event = Color(0xFFE46A9F),
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
