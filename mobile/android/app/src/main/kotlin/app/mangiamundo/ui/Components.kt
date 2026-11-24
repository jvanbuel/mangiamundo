package app.mangiamundo.ui

import android.content.ActivityNotFoundException
import android.content.Intent
import androidx.core.net.toUri
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Diamond
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import java.time.LocalDate
import app.mangiamundo.core.Status

private val CardShape = RoundedCornerShape(14.dp)

/** A panel with a solid outline and a heavier lower edge, rather than a soft tinted card. */
@Composable
internal fun Card(modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    val colors = LocalAppColors.current
    Box(
        modifier
            .fillMaxWidth()
            .padding(vertical = 6.dp)
            .clip(CardShape)
            .background(colors.line)
            .padding(start = 2.dp, top = 2.dp, end = 2.dp, bottom = 4.dp)
            .clip(RoundedCornerShape(14.dp))
            .background(colors.panel)
            .padding(16.dp),
    ) { content() }
}

/** A solid, pressable-looking button face on a darker lower edge. */
@Composable
fun ChunkyButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    face: Color = LocalAppColors.current.accent,
    content: Color = LocalAppColors.current.onAccent,
    enabled: Boolean = true,
    icon: ImageVector? = null,
) {
    val shape = RoundedCornerShape(12.dp)
    Box(
        modifier
            .alpha(if (enabled) 1f else 0.6f)
            .clip(shape)
            .background(lerp(face, Color.Black, 0.3f))
            .padding(bottom = 3.dp)
            .clip(shape)
            .background(face)
            .clickable(enabled = enabled, onClick = onClick)
            .padding(horizontal = 14.dp, vertical = 9.dp),
        contentAlignment = Alignment.Center,
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            if (icon != null) Icon(icon, contentDescription = null, tint = content, modifier = Modifier.size(18.dp))
            Text(text, color = content, style = MaterialTheme.typography.labelLarge)
        }
    }
}

@Composable
internal fun Bar(fraction: Float, modifier: Modifier = Modifier, color: Color = LocalAppColors.current.accent) =
    Bar(fraction, modifier, SolidColor(color))

/** A progress bar; the empty part is outlined, so it shows even at zero. */
@Composable
internal fun Bar(fraction: Float, modifier: Modifier = Modifier, brush: Brush) {
    val colors = LocalAppColors.current
    val shape = RoundedCornerShape(50)
    Box(modifier.height(6.dp).clip(shape).background(colors.track).border(1.dp, colors.trackEdge, shape)) {
        Box(Modifier.fillMaxHeight().fillMaxWidth(fraction.coerceIn(0f, 1f)).clip(shape).background(brush))
    }
}

internal fun fraction(part: Int, whole: Int) = if (whole == 0) 0f else part.toFloat() / whole

/** A country's state by shape as well as colour: a blue ring, half-filled gold, full platinum, a diamond. */
@Composable
fun StatusMark(status: Status, size: Dp = 14.dp) {
    val colors = LocalAppColors.current
    val ring = when (status) {
        Status.AVAILABLE -> colors.available
        Status.DISCOVERED -> colors.discovered
        Status.PLATINUM -> colors.platinumEdge
        Status.COMPLETED -> colors.completedEdge
    }
    val fill = when (status) {
        Status.AVAILABLE -> SolidColor(Color.Transparent)
        Status.DISCOVERED -> Brush.verticalGradient(0.5f to Color.Transparent, 0.5f to colors.discovered)
        Status.PLATINUM -> colors.platinumMetal
        Status.COMPLETED -> colors.diamond
    }
    Box(Modifier.size(size).clip(CircleShape).background(fill).border(2.dp, ring, CircleShape), contentAlignment = Alignment.Center) {
        if (status == Status.COMPLETED) {
            Icon(Icons.Rounded.Diamond, contentDescription = null, tint = colors.onCompleted, modifier = Modifier.size(size * 0.55f))
        }
    }
}

/** What players call each status. */
val Status.label: String
    get() = when (this) {
        Status.AVAILABLE -> "To discover"
        Status.DISCOVERED -> "Gold"
        Status.PLATINUM -> "Platinum"
        Status.COMPLETED -> "Diamond"
    }

@Composable
fun Legend(modifier: Modifier = Modifier) {
    val colors = LocalAppColors.current
    Row(
        modifier
            .clip(RoundedCornerShape(10.dp))
            .background(colors.panel.copy(alpha = 0.92f))
            .border(1.dp, colors.line, RoundedCornerShape(10.dp))
            .padding(horizontal = 10.dp, vertical = 6.dp),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        for (status in Status.entries) {
            val label = status.label
            Row(verticalAlignment = Alignment.CenterVertically) {
                StatusMark(status, size = 11.dp)
                Text(" $label", color = colors.muted, fontSize = 11.sp)
            }
        }
    }
}

@Composable
fun Note(text: String) {
    Text(text, color = LocalAppColors.current.muted, modifier = Modifier.padding(vertical = 12.dp))
}

@Composable
fun ErrorCard(message: String, onRetry: () -> Unit) {
    Card {
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Text(message, color = LocalAppColors.current.text)
            Button(onClick = onRetry) { Text("Try again") }
        }
    }
}

internal fun distance(meters: Int) = if (meters < 1000) "${meters / 10 * 10} m" else "%.1f km".format(meters / 1000.0)

/** Opens a URL in the browser or the app that handles it, or says there is none. */
internal fun android.content.Context.open(url: String) =
    try {
        startActivity(Intent(Intent.ACTION_VIEW, url.toUri()))
    } catch (_: ActivityNotFoundException) {
        android.widget.Toast.makeText(this, "No app on this phone can open that link.", android.widget.Toast.LENGTH_SHORT).show()
    }

internal fun parseDay(day: String) = java.time.LocalDate.parse(day)
internal val shortDate = java.time.format.DateTimeFormatter.ofPattern("d MMM")
