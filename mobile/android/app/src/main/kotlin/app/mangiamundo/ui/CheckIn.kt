package app.mangiamundo.ui

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Place
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import java.time.LocalDate
import app.mangiamundo.core.Place

@Composable
fun CheckInButton(place: Place, busy: Boolean, today: LocalDate = LocalDate.now(), onToggle: () -> Unit) {
    val colors = LocalAppColors.current
    var day by remember(today) { mutableStateOf(today) }
    val visited = place.visitedToday(day)
    // Past midnight the label changes first, so a tap never does the opposite of what it says.
    val onClick = {
        val now = LocalDate.now()
        if (place.visitedToday(now) == visited) onToggle() else day = now
    }
    if (visited) {
        ChunkyButton("Visited", onClick, face = colors.discovered, content = colors.onDiscovered, enabled = !busy, icon = Icons.Rounded.Check)
    } else {
        ChunkyButton(if (place.visited) "Check in again" else "Check in", onClick, enabled = !busy)
    }
}
