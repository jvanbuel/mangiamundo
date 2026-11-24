package app.mangiamundo.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.TrackChanges
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import java.time.LocalDate
import app.mangiamundo.core.Badge
import app.mangiamundo.core.EventProgress
import app.mangiamundo.core.MonthlyQuests
import app.mangiamundo.core.EventStatus

@Composable
private fun LimitedTime(events: List<EventProgress>, today: java.time.LocalDate) {
    val colors = LocalAppColors.current
    Column(Modifier.padding(top = 12.dp, bottom = 8.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text("Limited time", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Black, color = colors.text)
        for (e in events) {
            val timing = if (e.status == EventStatus.UPCOMING) {
                "Starts ${parseDay(e.starts).format(shortDate)}"
            } else {
                val left = java.time.temporal.ChronoUnit.DAYS.between(today, parseDay(e.ends))
                if (left <= 0) "Last day" else "${left + 1} days left"
            }
            Row(verticalAlignment = Alignment.Top) {
                EventMedal(earned = e.status == EventStatus.ACTIVE, size = 52.dp)
                Column(Modifier.padding(start = 12.dp).weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(e.title, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.ExtraBold, color = colors.text, modifier = Modifier.weight(1f))
                        Text(
                            timing,
                            style = MaterialTheme.typography.labelMedium,
                            color = colors.accentText,
                            modifier = Modifier.clip(RoundedCornerShape(50)).background(colors.event.copy(alpha = 0.16f)).padding(horizontal = 8.dp, vertical = 2.dp),
                        )
                    }
                    Text(e.description, style = MaterialTheme.typography.bodySmall, color = colors.muted)
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Bar(fraction(e.progress, e.target), Modifier.weight(1f), color = colors.event)
                        Text("  ${e.progress} / ${e.target}", style = MaterialTheme.typography.bodySmall, color = colors.muted)
                    }
                    if (e.placesNearby == 0) Text("No places nearby for this one yet.", style = MaterialTheme.typography.bodySmall, color = colors.muted)
                }
            }
        }
    }
}

@Composable
private fun ThisMonth(monthly: MonthlyQuests) {
    val colors = LocalAppColors.current
    Column(Modifier.padding(top = 12.dp, bottom = 8.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text("This month", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Black, color = colors.text, modifier = Modifier.weight(1f))
            Text("Until ${parseDay(monthly.ends).format(shortDate)}", style = MaterialTheme.typography.labelMedium, color = colors.muted)
        }
        for (q in monthly.quests) {
            Row(verticalAlignment = Alignment.Top) {
                MedalFace(Icons.Rounded.TrackChanges, MedalFace.BLUE, q.done, 52.dp)
                Column(Modifier.padding(start = 12.dp).weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(q.title, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.ExtraBold, color = colors.text, modifier = Modifier.weight(1f))
                        Text(if (q.done) "Done" else "+${q.xp} XP", style = MaterialTheme.typography.labelMedium, color = colors.muted)
                    }
                    Text(q.description, style = MaterialTheme.typography.bodySmall, color = colors.muted)
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Bar(fraction(q.progress, q.target), Modifier.weight(1f))
                        Text("  ${q.progress} / ${q.target}", style = MaterialTheme.typography.bodySmall, color = colors.muted)
                    }
                }
            }
        }
    }
}

private class Tile(val key: String, val title: String, val detail: String, val medal: @Composable () -> Unit)

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun BadgeGrid(
    badges: List<Badge>,
    events: List<EventProgress> = emptyList(),
    quests: MonthlyQuests? = null,
    today: java.time.LocalDate = java.time.LocalDate.now(),
) {
    val colors = LocalAppColors.current
    if (quests != null) ThisMonth(quests)
    val running = events.filter { (it.status == EventStatus.UPCOMING || it.status == EventStatus.ACTIVE) && !it.earned }
    if (running.isNotEmpty()) LimitedTime(running, today)
    val tiles = events.filter { it.earned }.map { e ->
        Tile(e.id, e.title, "${parseDay(e.starts).format(shortDate)} – ${parseDay(e.ends).format(shortDate)}") { EventMedal(earned = true) }
    } + badges.sortedByDescending { it.earned }.map { b ->
        Tile(b.id, b.title, if (b.earned) "Earned" else "${b.progress} / ${b.target}") { Medal(b.kind, b.earned) }
    }
    FlowRow(
        Modifier.fillMaxWidth().padding(top = 12.dp),
        maxItemsInEachRow = 3,
        verticalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        for (tile in tiles) {
            Column(
                Modifier.weight(1f).semantics(mergeDescendants = true) {},
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                tile.medal()
                Text(
                    tile.title,
                    style = MaterialTheme.typography.titleSmall,
                    fontWeight = FontWeight.ExtraBold,
                    color = colors.text,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.padding(top = 8.dp, start = 4.dp, end = 4.dp),
                )
                Text(
                    tile.detail,
                    style = MaterialTheme.typography.bodySmall,
                    color = colors.muted,
                )
            }
        }
        // Keep the last row's medals the same width as the others.
        repeat((3 - tiles.size % 3) % 3) { Spacer(Modifier.weight(1f)) }
    }
}
