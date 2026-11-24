package app.mangiamundo.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import app.mangiamundo.core.Country
import app.mangiamundo.core.UnknownPlace

/** Restaurants OpenStreetMap gives no cuisine: the user says which country's food each serves. */
@Composable
fun UnknownFoodSheet(
    places: List<UnknownPlace>,
    countries: List<Country>,
    answering: String?,
    onAnswer: (UnknownPlace, country: String?) -> Unit,
) {
    val colors = LocalAppColors.current
    // Only countries with a cuisine can be the food a place serves.
    val choices = remember(countries) { countries.filter { it.cuisine != null }.sortedBy { it.name } }
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Text("What food do they serve?", style = MaterialTheme.typography.titleLarge, color = colors.text)
        for (place in places) {
            var choosing by remember { mutableStateOf(false) }
            Column(
                Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(colors.panelRaised).padding(12.dp),
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Text(place.name, color = colors.text, fontWeight = FontWeight.Bold)
                Text(
                    listOfNotNull(distance(place.distanceM), place.address).joinToString(" · "),
                    style = MaterialTheme.typography.bodySmall,
                    color = colors.muted,
                )
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Box {
                        ChunkyButton(
                            if (answering == place.id) "Saving…" else "Food from…",
                            onClick = { choosing = true },
                            enabled = answering == null,
                        )
                        DropdownMenu(expanded = choosing, onDismissRequest = { choosing = false }) {
                            for (c in choices) {
                                DropdownMenuItem(text = { Text("${c.flag} ${c.name}") }, onClick = { choosing = false; onAnswer(place, c.code) })
                            }
                        }
                    }
                    ChunkyButton(
                        "Nothing specific",
                        onClick = { onAnswer(place, null) },
                        face = colors.panel,
                        content = colors.text,
                        enabled = answering == null,
                    )
                }
            }
        }
        Text(
            "A country you pick counts for you right away. We ask OpenStreetMap's mappers to add it with a note.",
            style = MaterialTheme.typography.bodySmall,
            color = colors.muted,
        )
    }
}
