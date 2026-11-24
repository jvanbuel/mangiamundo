package app.mangiamundo.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Share
import androidx.compose.material.icons.rounded.Diamond
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import app.mangiamundo.core.Discover

@Composable
private fun StatRow(label: String, value: Int, total: Int? = null) {
    val colors = LocalAppColors.current
    Row(verticalAlignment = Alignment.Bottom) {
        Text(label, style = MaterialTheme.typography.titleSmall, color = colors.text, modifier = Modifier.weight(1f))
        Text(
            buildAnnotatedString {
                withStyle(SpanStyle(fontWeight = FontWeight.Bold, color = colors.text)) { append("$value") }
                if (total != null) append(" of $total")
            },
            style = MaterialTheme.typography.bodyMedium,
            color = colors.muted,
        )
    }
}

@Composable
fun Passport(discover: Discover, sharing: Boolean = false, onShare: (() -> Unit)? = null) {
    val colors = LocalAppColors.current
    val profile = discover.profile
    Card {
        Column(verticalArrangement = Arrangement.spacedBy(14.dp)) {
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Row(verticalAlignment = Alignment.Bottom) {
                    Text("Level ${profile.level}", style = MaterialTheme.typography.titleSmall, color = colors.text, modifier = Modifier.weight(1f))
                    Text("${profile.xp} / ${profile.nextLevelXp} XP", style = MaterialTheme.typography.bodyMedium, color = colors.muted)
                }
                Bar(fraction(profile.xp - profile.levelXp, profile.nextLevelXp - profile.levelXp), Modifier.fillMaxWidth())
            }
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                StatRow("Countries tasted", discover.world.discovered, discover.world.available)
                Bar(fraction(discover.world.discovered, discover.world.available), Modifier.fillMaxWidth())
            }
            HorizontalDivider(color = colors.line)
            Row {
                Column(Modifier.weight(1f)) {
                    Text("Places visited", style = MaterialTheme.typography.bodySmall, color = colors.muted)
                    Text("${discover.placesVisited} of ${discover.places}", style = MaterialTheme.typography.titleSmall, color = colors.text)
                }
                Column(Modifier.weight(1f)) {
                    Text("Diamond countries", style = MaterialTheme.typography.bodySmall, color = colors.muted)
                    Text("${discover.world.completed}", style = MaterialTheme.typography.titleSmall, color = colors.text)
                }
                Column(Modifier.weight(1f)) {
                    Text("Month streak", style = MaterialTheme.typography.bodySmall, color = colors.muted)
                    Text("${profile.streak} · best ${profile.bestStreak}", style = MaterialTheme.typography.titleSmall, color = colors.text)
                }
            }
            if (onShare != null && discover.placesVisited > 0) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    ChunkyButton(if (sharing) "Preparing…" else "Share my map", onClick = onShare, enabled = !sharing, icon = Icons.Rounded.Share)
                    Text("Makes a public link to your map, level and badges.", style = MaterialTheme.typography.bodySmall, color = colors.muted)
                }
            }
        }
    }
}
