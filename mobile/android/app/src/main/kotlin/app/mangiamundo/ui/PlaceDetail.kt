package app.mangiamundo.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Casino
import androidx.compose.material.icons.rounded.Share
import androidx.compose.material.icons.rounded.Place
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import app.mangiamundo.DetailVisits
import java.time.LocalDate
import app.mangiamundo.core.Country
import app.mangiamundo.core.Place

/** OSM cuisine tags read as words: "ethiopian;coffee_shop" becomes "ethiopian, coffee shop". */
private fun cuisine(raw: String) = raw.replace('_', ' ').split(';').joinToString(", ") { it.trim() }

private val visitDate = java.time.format.DateTimeFormatter.ofLocalizedDate(java.time.format.FormatStyle.FULL)

/** Everything about a place, with all of the user's visits. Shown in a bottom sheet. */
@Composable
fun PlaceDetail(
    place: Place,
    country: (String) -> Country?,
    busy: Boolean,
    visits: DetailVisits = DetailVisits.LOADED,
    onRetry: () -> Unit = {},
    onReportClosed: () -> Unit = {},
    refreshing: Boolean = false,
    onRefresh: () -> Unit = {},
    onEdit: () -> Unit = {},
    /** Shares one visit; no share buttons without it. */
    onShareVisit: ((Long) -> Unit)? = null,
    sharing: Boolean = false,
    /** The place came from a spin: offer to spin again or put it off. */
    spun: Boolean = false,
    spinning: Boolean = false,
    onSpin: () -> Unit = {},
    onNotToday: () -> Unit = {},
    today: LocalDate = LocalDate.now(),
    onToggle: () -> Unit,
) {
    val colors = LocalAppColors.current
    val context = LocalContext.current
    val body = MaterialTheme.typography.bodyMedium
    // Reports cannot be taken back, so the link only asks first.
    var confirmingClosed by remember { mutableStateOf(false) }
    if (confirmingClosed) {
        AlertDialog(
            onDismissRequest = { confirmingClosed = false },
            title = { Text("Has ${place.name} closed for good?") },
            text = { Text("It will be hidden for you, and for everyone once someone else reports it too.") },
            confirmButton = {
                TextButton(onClick = { confirmingClosed = false; onReportClosed() }) { Text("Yes, it closed") }
            },
            dismissButton = { TextButton(onClick = { confirmingClosed = false }) { Text("Cancel") } },
        )
    }
    Column(Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, bottom = 32.dp)) {
        Text(place.name, style = MaterialTheme.typography.headlineSmall, color = colors.text, modifier = Modifier.semantics { heading() })
        place.cuisine?.let { Text(cuisine(it), style = body, color = colors.muted) }
        Text(
            place.countries.joinToString("   ") { code -> country(code)?.let { "${it.flag} ${it.name}" } ?: code },
            style = body,
            fontWeight = FontWeight.Bold,
            color = colors.text,
            modifier = Modifier.padding(top = 2.dp),
        )
        Row(
            Modifier.padding(vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            CheckInButton(place, busy, today, onToggle)
            PlaceLinks(place)
        }
        if (spun) {
            Row(Modifier.padding(bottom = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                ChunkyButton("Spin again", onSpin, face = colors.track, content = colors.text, enabled = !spinning, icon = Icons.Rounded.Casino)
                ChunkyButton("Not today", onNotToday, face = colors.track, content = colors.text, enabled = !spinning)
            }
        }
        val where = listOfNotNull(place.address, place.distanceM.takeIf { it > 0 }?.let { "${distance(it)} away" })
        if (where.isNotEmpty()) Text(where.joinToString(" · "), style = body, color = colors.text)

        place.hours?.let { hours ->
            DetailHeading("Opening hours")
            hours.status?.let {
                Text(it.text, style = body, fontWeight = FontWeight.Bold, color = if (it.open) colors.text else colors.muted)
            }
            val week = hours.week
            if (week == null) {
                Text(hours.raw, style = body, color = colors.muted)
            } else {
                for (day in week) {
                    val weight = if (day.today) FontWeight.ExtraBold else FontWeight.Normal
                    Row(Modifier.padding(vertical = 2.dp)) {
                        Text(day.name, style = body, fontWeight = weight, color = colors.text, modifier = Modifier.width(110.dp))
                        Text(day.open.joinToString(", ").ifEmpty { "Closed" }, style = body, fontWeight = weight, color = colors.text)
                    }
                }
            }
        }

        DetailHeading("Your visits")
        // Until the details load, the visits are the card's, which has only the latest one.
        when (visits) {
            DetailVisits.LOADING -> Text("Loading visits…", style = body, color = colors.muted)
            DetailVisits.FAILED -> Text(
                "Could not load all visits. Try again",
                style = body,
                color = colors.accent,
                modifier = Modifier.clickable(onClick = onRetry).padding(vertical = 6.dp),
            )
            DetailVisits.LOADED -> if (place.visits.isEmpty()) Text("Not visited yet.", style = body, color = colors.muted)
        }
        for (visit in place.visits) {
            val date = java.time.Instant.ofEpochSecond(visit.at).atZone(java.time.ZoneId.systemDefault()).format(visitDate)
            Row(Modifier.padding(vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(
                    date + if (visit.verified) " · checked in on the spot" else "",
                    style = body,
                    color = colors.text,
                    modifier = Modifier.weight(1f),
                )
                if (onShareVisit != null) {
                    ChunkyButton("Share", onClick = { onShareVisit(visit.id) }, enabled = !sharing, icon = Icons.Rounded.Share)
                }
            }
            HorizontalDivider(color = colors.line)
        }

        val small = MaterialTheme.typography.bodySmall
        Text(
            "Closed for good?",
            style = small,
            color = colors.muted,
            modifier = Modifier.padding(top = 28.dp).clickable(enabled = !busy) { confirmingClosed = true },
        )
        place.osmUrl?.let { url ->
            // A phone-friendly OSM editor when one is installed, otherwise the website.
            var editor by remember(context) { mutableStateOf(context.editorApp()) }
            val lifecycleOwner = LocalLifecycleOwner.current
            DisposableEffect(lifecycleOwner, context) {
                val observer = LifecycleEventObserver { _, event ->
                    if (event == Lifecycle.Event.ON_RESUME) editor = context.editorApp()
                }
                lifecycleOwner.lifecycle.addObserver(observer)
                onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
            }
            Text(
                "Something wrong? Edit " + (editor?.let { "in ${it.label}" } ?: "on OpenStreetMap"),
                style = small,
                color = colors.muted,
                // Checked again on tap: the app may have been uninstalled since the label was drawn.
                modifier = Modifier.padding(top = 4.dp).clickable {
                    onEdit()
                    context.edit(place, context.editorApp(), url)
                },
            )
            Text(
                if (refreshing) "Refreshing from OpenStreetMap…" else "Refresh from OpenStreetMap",
                style = small,
                color = colors.muted,
                modifier = Modifier.padding(top = 4.dp).clickable(enabled = !refreshing, onClick = onRefresh),
            )
            Text(
                "Data © OpenStreetMap contributors",
                style = small,
                color = colors.muted,
                modifier = Modifier.padding(top = 4.dp).clickable { context.open("https://www.openstreetmap.org/copyright") },
            )
        }
    }
}

@Composable
private fun DetailHeading(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.titleSmall,
        fontWeight = FontWeight.ExtraBold,
        color = LocalAppColors.current.text,
        modifier = Modifier.padding(top = 18.dp, bottom = 4.dp).semantics { heading() },
    )
}
