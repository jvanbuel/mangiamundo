package app.mangiamundo.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AddLocationAlt
import androidx.compose.material.icons.rounded.Celebration
import androidx.compose.material.icons.rounded.EmojiEvents
import androidx.compose.material.icons.rounded.Flag
import androidx.compose.material.icons.rounded.LocalFireDepartment
import androidx.compose.material.icons.rounded.Map
import androidx.compose.material.icons.rounded.Public
import androidx.compose.material.icons.rounded.Restaurant
import androidx.compose.material.icons.rounded.Verified
import androidx.compose.material3.Icon
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.mangiamundo.core.BadgeKind
import app.mangiamundo.core.Celebration

private fun BadgeKind.icon(): ImageVector = when (this) {
    BadgeKind.VISITS -> Icons.Rounded.Restaurant
    BadgeKind.COUNTRIES -> Icons.Rounded.Flag
    BadgeKind.COMPLETED -> Icons.Rounded.Verified
    BadgeKind.CONTINENT -> Icons.Rounded.Map
    BadgeKind.WORLD -> Icons.Rounded.Public
    BadgeKind.PIONEER -> Icons.Rounded.AddLocationAlt
    BadgeKind.STREAK -> Icons.Rounded.LocalFireDepartment
    BadgeKind.EVERYTHING, BadgeKind.UNKNOWN -> Icons.Rounded.EmojiEvents
}

/** Gold for tasting, diamond for finishing, blue for the rest. */
enum class MedalFace { GOLD, DIAMOND, BLUE, EVENT }

private fun BadgeKind.face(): MedalFace = when (this) {
    BadgeKind.VISITS, BadgeKind.COUNTRIES, BadgeKind.STREAK -> MedalFace.GOLD
    BadgeKind.COMPLETED, BadgeKind.CONTINENT, BadgeKind.WORLD, BadgeKind.EVERYTHING -> MedalFace.DIAMOND
    BadgeKind.PIONEER, BadgeKind.UNKNOWN -> MedalFace.BLUE
}

/** A solid token with a darker lower edge, so it reads as something you collect. */
@Composable
internal fun MedalFace(icon: ImageVector, face: MedalFace, earned: Boolean, size: Dp) {
    val colors = LocalAppColors.current
    val fill: Brush = when {
        !earned -> SolidColor(colors.line)
        face == MedalFace.GOLD -> SolidColor(colors.discovered)
        face == MedalFace.DIAMOND -> colors.diamond
        face == MedalFace.EVENT -> SolidColor(colors.event)
        else -> SolidColor(colors.accent)
    }
    val edge = when {
        !earned -> lerp(colors.line, Color.Black, 0.3f)
        face == MedalFace.GOLD -> colors.discoveredEdge
        face == MedalFace.DIAMOND -> colors.completedEdge
        face == MedalFace.EVENT -> lerp(colors.event, Color.Black, 0.3f)
        else -> lerp(colors.accent, Color.Black, 0.3f)
    }
    val ink = when {
        !earned -> colors.muted
        face == MedalFace.GOLD -> colors.onDiscovered
        face == MedalFace.DIAMOND -> colors.onCompleted
        else -> colors.onAccent
    }
    Box(
        Modifier
            .size(size)
            .clip(CircleShape)
            .background(edge)
            .padding(bottom = size * 0.07f)
            .clip(CircleShape)
            .background(fill),
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, contentDescription = null, tint = ink, modifier = Modifier.size(size * 0.46f))
    }
}

@Composable
fun Medal(kind: BadgeKind, earned: Boolean, size: Dp = 64.dp) = MedalFace(kind.icon(), kind.face(), earned, size)

/** The medal of a limited-time event. */
@Composable
fun EventMedal(earned: Boolean, size: Dp = 64.dp) = MedalFace(Icons.Rounded.Celebration, MedalFace.EVENT, earned, size)
