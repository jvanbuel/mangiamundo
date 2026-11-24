package app.mangiamundo.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.Place
import androidx.compose.material3.Icon
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.mangiamundo.core.Country
import app.mangiamundo.core.CountryProgress
import app.mangiamundo.core.Place
import app.mangiamundo.core.Status

@Composable
fun CountryHeader(progress: CountryProgress, info: Country?, onBack: () -> Unit) {
    val colors = LocalAppColors.current
    Column(Modifier.padding(top = 4.dp, bottom = 8.dp)) {
        TextButton(onClick = onBack) {
            Icon(Icons.AutoMirrored.Rounded.ArrowBack, contentDescription = null, tint = colors.muted, modifier = Modifier.size(18.dp))
            Text(" All countries", color = colors.muted, style = MaterialTheme.typography.labelLarge)
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(info?.flag.orEmpty(), fontSize = 44.sp)
            Column(Modifier.padding(start = 14.dp)) {
                Text(info?.name ?: progress.code, style = MaterialTheme.typography.headlineSmall, color = colors.text)
                Text(
                    buildString {
                        if (progress.status != Status.AVAILABLE) append("${progress.status.label} · ")
                        append("${progress.visited} of ${progress.places} ${if (progress.places == 1) "place" else "places"} visited")
                        if (progress.status != Status.COMPLETED) append(" · ${progress.goal - progress.visited} more for diamond")
                    },
                    fontSize = 13.sp,
                    color = colors.muted,
                )
            }
        }
        Bar(
            fraction(progress.visited, progress.goal).coerceAtMost(1f),
            Modifier.fillMaxWidth().padding(top = 10.dp),
            brush = when (progress.status) {
                Status.AVAILABLE -> SolidColor(colors.accent)
                Status.DISCOVERED -> SolidColor(colors.discovered)
                Status.PLATINUM -> colors.platinumMetal
                Status.COMPLETED -> colors.diamondBar
            },
        )
    }
}

@Composable
fun PlaceGroupHeader(openNow: Boolean?) {
    Text(
        when (openNow) {
            true -> "OPEN NOW"
            false -> "CLOSED"
            null -> "HOURS UNKNOWN"
        },
        style = MaterialTheme.typography.labelMedium,
        fontWeight = FontWeight.ExtraBold,
        color = LocalAppColors.current.muted,
        modifier = Modifier.padding(top = 18.dp, bottom = 2.dp),
    )
}

/** A place in a country's list. Tapping it (outside its links and button) opens the details. */
@Composable
fun PlaceRow(place: Place, busy: Boolean, onOpen: () -> Unit, onToggle: () -> Unit) {
    val colors = LocalAppColors.current
    Column {
        Row(
            Modifier
                .fillMaxWidth()
                .clickable(onClickLabel = "Show details", onClick = onOpen)
                .padding(vertical = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f).padding(end = 12.dp)) {
                Text(place.name, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.ExtraBold, color = colors.text)
                place.hoursText?.let {
                    Text(
                        it,
                        style = MaterialTheme.typography.bodyMedium,
                        fontWeight = FontWeight.Bold,
                        color = if (place.openNow == true) colors.text else colors.muted,
                    )
                }
                Text(
                    distance(place.distanceM) + (place.address?.let { " · $it" } ?: "") +
                        if (place.verified) " · checked in on the spot" else "",
                    style = MaterialTheme.typography.bodyMedium,
                    color = colors.muted,
                )
                Box(Modifier.padding(top = 4.dp)) { PlaceLinks(place) }
            }
            CheckInButton(place, busy, onToggle = onToggle)
        }
        HorizontalDivider(color = colors.line)
    }
}
