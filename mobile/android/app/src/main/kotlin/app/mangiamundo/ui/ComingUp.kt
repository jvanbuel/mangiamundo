package app.mangiamundo.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Celebration
import androidx.compose.material.icons.rounded.Notifications
import androidx.compose.material.icons.rounded.NotificationsActive
import androidx.compose.material3.Icon
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import java.time.LocalDate
import app.mangiamundo.core.Celebration
import app.mangiamundo.core.Country

@Composable
fun ComingUp(
    celebrations: List<Celebration>,
    remindersOn: Boolean,
    onToggleReminders: () -> Unit,
    country: (String) -> Country?,
    onOpen: (String) -> Unit,
    today: java.time.LocalDate = java.time.LocalDate.now(),
) {
    val colors = LocalAppColors.current
    Card {
        Column {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("Coming up", style = MaterialTheme.typography.titleLarge, color = colors.text, modifier = Modifier.weight(1f))
                FilterChip(
                    selected = remindersOn,
                    onClick = onToggleReminders,
                    label = { Text(if (remindersOn) "Reminders on" else "Remind me") },
                    leadingIcon = {
                        Icon(
                            if (remindersOn) Icons.Rounded.NotificationsActive else Icons.Rounded.Notifications,
                            contentDescription = null,
                            modifier = Modifier.size(18.dp),
                        )
                    },
                )
            }
            for ((index, c) in celebrations.take(4).withIndex()) {
                if (index > 0) HorizontalDivider(color = colors.line)
                val starts = parseDay(c.starts)
                val days = java.time.temporal.ChronoUnit.DAYS.between(today, starts)
                val whenText = when {
                    days <= 0 -> if (parseDay(c.ends).isAfter(today)) "On now" else "Today"
                    days == 1L -> "Tomorrow"
                    else -> "In $days days"
                }
                val total = c.nearby.sumOf { it.places }
                val flags = c.nearby.joinToString(" ") { country(it.code)?.flag.orEmpty() }
                Row(
                    Modifier
                        .fillMaxWidth()
                        .clickable { c.nearby.firstOrNull()?.let { onOpen(it.code) } }
                        .padding(vertical = 10.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    CalendarPage(starts)
                    Column(Modifier.padding(start = 12.dp)) {
                        Text(c.title, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.ExtraBold, color = colors.text)
                        Text(
                            "$whenText${if (c.approximate) " (date may shift)" else ""} · $flags $total ${if (total == 1) "place" else "places"} nearby",
                            style = MaterialTheme.typography.bodyMedium,
                            color = colors.muted,
                        )
                    }
                }
            }
        }
    }
}

/** A little tear-off calendar page. */
@Composable
private fun CalendarPage(day: java.time.LocalDate) {
    val colors = LocalAppColors.current
    val shape = RoundedCornerShape(10.dp)
    Column(
        Modifier
            .width(48.dp)
            .clip(shape)
            .background(colors.line)
            .padding(start = 2.dp, top = 2.dp, end = 2.dp, bottom = 3.dp)
            .clip(RoundedCornerShape(8.dp))
            .background(colors.panel),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            day.format(java.time.format.DateTimeFormatter.ofPattern("MMM")).uppercase(),
            style = MaterialTheme.typography.labelSmall,
            color = colors.onAccent,
            textAlign = TextAlign.Center,
            modifier = Modifier.fillMaxWidth().background(colors.event).padding(vertical = 1.dp),
        )
        Text("${day.dayOfMonth}", style = MaterialTheme.typography.titleLarge, color = colors.text)
    }
}
