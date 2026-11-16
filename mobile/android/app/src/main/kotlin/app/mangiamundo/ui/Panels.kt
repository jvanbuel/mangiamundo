package app.mangiamundo.ui

import android.content.Intent
import androidx.core.net.toUri
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.FilterChip
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.mangiamundo.AppViewModel
import app.mangiamundo.SavedLocation
import app.mangiamundo.core.Badge
import app.mangiamundo.core.Country
import app.mangiamundo.core.CountryProgress
import app.mangiamundo.core.Discover
import app.mangiamundo.core.GeoResult
import app.mangiamundo.core.GroupProgress
import app.mangiamundo.core.Place
import app.mangiamundo.core.Status
import kotlinx.coroutines.launch

private val CardShape = RoundedCornerShape(14.dp)

@Composable
private fun Card(modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    val colors = LocalAppColors.current
    Box(
        modifier
            .fillMaxWidth()
            .padding(vertical = 6.dp)
            .clip(CardShape)
            .background(colors.panelRaised.copy(alpha = 0.55f))
            .border(1.dp, colors.line, CardShape)
            .padding(16.dp),
    ) { content() }
}

@Composable
private fun Bar(fraction: Float, modifier: Modifier = Modifier, color: Color = LocalAppColors.current.accent) {
    val shape = RoundedCornerShape(50)
    Box(modifier.height(6.dp).clip(shape).background(LocalAppColors.current.line)) {
        Box(Modifier.fillMaxHeight().fillMaxWidth(fraction.coerceIn(0f, 1f)).clip(shape).background(color))
    }
}

private fun fraction(part: Int, whole: Int) = if (whole == 0) 0f else part.toFloat() / whole

@Composable
private fun statusColor(status: Status): Color = with(LocalAppColors.current) {
    when (status) {
        Status.AVAILABLE -> available
        Status.DISCOVERED -> discovered
        Status.COMPLETED -> completed
    }
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
        for ((color, label) in listOf(
            colors.available to "To discover",
            colors.discovered to "Tasted",
            colors.completed to "Completed",
        )) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(9.dp).clip(RoundedCornerShape(3.dp)).background(color))
                Text(" $label", color = colors.muted, fontSize = 11.sp)
            }
        }
    }
}

@Composable
fun Header(location: SavedLocation?, radiusKm: Int, onChange: () -> Unit) {
    val colors = LocalAppColors.current
    Column(Modifier.padding(bottom = 6.dp)) {
        Text("Mangiamundo", style = MaterialTheme.typography.titleLarge, color = colors.text)
        if (location != null) {
            Row(
                Modifier
                    .padding(top = 10.dp)
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(10.dp))
                    .border(1.dp, colors.line, RoundedCornerShape(10.dp))
                    .clickable(onClick = onChange)
                    .padding(horizontal = 12.dp, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    location.name ?: "Your location",
                    fontWeight = FontWeight.Bold,
                    color = colors.text,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f, fill = false),
                )
                Text("  ·  $radiusKm km", color = colors.muted)
                Spacer(Modifier.weight(1f))
                Text("Change", color = colors.accent, fontSize = 13.sp)
            }
        }
    }
}

@Composable
fun Welcome(picker: @Composable () -> Unit) {
    val colors = LocalAppColors.current
    Card {
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Text(
                "Eat your way around the world without leaving town.",
                style = MaterialTheme.typography.titleLarge,
                color = colors.text,
            )
            Text(
                "Tell us where you are and we will light up every country whose kitchen you can visit nearby.",
                color = colors.muted,
            )
            picker()
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

@Composable
fun LocationPicker(vm: AppViewModel, onUseGps: () -> Unit, onDone: () -> Unit) {
    val colors = LocalAppColors.current
    val scope = rememberCoroutineScope()
    var query by rememberSaveable { mutableStateOf("") }
    var results by remember { mutableStateOf<List<GeoResult>?>(null) }
    var searching by remember { mutableStateOf(false) }
    var failure by remember { mutableStateOf<String?>(null) }

    val search = {
        if (query.trim().length >= 2 && !searching) {
            searching = true
            failure = null
            scope.launch {
                try {
                    results = vm.search(query.trim())
                } catch (e: Exception) {
                    failure = e.message
                }
                searching = false
            }
        }
    }

    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Button(
            onClick = {
                onUseGps()
                onDone()
            },
            enabled = !vm.locating,
            modifier = Modifier.fillMaxWidth(),
        ) { Text(if (vm.locating) "Finding you…" else "Use my current location") }
        vm.locateError?.let { Text(it, color = colors.danger, fontSize = 13.sp) }

        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedTextField(
                value = query,
                onValueChange = { query = it },
                placeholder = { Text("Search a city or address") },
                singleLine = true,
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                keyboardActions = KeyboardActions(onSearch = { search() }),
                modifier = Modifier.weight(1f),
            )
            OutlinedButton(onClick = { search() }, enabled = !searching) { Text(if (searching) "…" else "Search") }
        }
        failure?.let { Text(it, color = colors.danger, fontSize = 13.sp) }

        results?.let { found ->
            if (found.isEmpty()) Text("No matches. Try a different spelling.", color = colors.muted, fontSize = 13.sp)
            for (result in found) {
                Text(
                    result.name,
                    color = colors.text,
                    modifier = Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(8.dp))
                        .clickable {
                            vm.chooseLocation(result)
                            onDone()
                        }
                        .padding(10.dp),
                )
            }
        }

        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Text("Search radius", color = colors.muted, fontSize = 13.sp, modifier = Modifier.weight(1f))
            for (km in listOf(5, 10, 25)) {
                FilterChip(selected = vm.radiusKm == km, onClick = { vm.changeRadius(km) }, label = { Text("$km km") })
            }
        }
    }
}

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
fun Passport(discover: Discover) {
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
                    Text("Countries completed", style = MaterialTheme.typography.bodySmall, color = colors.muted)
                    Text("${discover.world.completed}", style = MaterialTheme.typography.titleSmall, color = colors.text)
                }
            }
            HorizontalDivider(color = colors.line)
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                for (continent in discover.continents) {
                    val empty = continent.available == 0
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(
                            continent.name,
                            style = MaterialTheme.typography.bodyMedium,
                            color = if (empty) colors.muted else colors.text,
                            modifier = Modifier.width(112.dp),
                        )
                        Bar(fraction(continent.discovered, continent.available), Modifier.weight(1f))
                        Text(
                            if (empty) "–" else "${continent.discovered}/${continent.available}",
                            style = MaterialTheme.typography.bodyMedium,
                            color = colors.muted,
                            modifier = Modifier.width(44.dp),
                            textAlign = TextAlign.End,
                        )
                    }
                }
            }
        }
    }
}

@Composable
fun ContinentHeader(continent: GroupProgress) {
    val colors = LocalAppColors.current
    Row(Modifier.fillMaxWidth().padding(top = 16.dp, bottom = 4.dp, start = 8.dp, end = 8.dp)) {
        Text(continent.name, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.ExtraBold, color = colors.text)
        Spacer(Modifier.weight(1f))
        Text("${continent.discovered}/${continent.available}", style = MaterialTheme.typography.bodyMedium, color = colors.muted)
    }
}

@Composable
fun CountryRow(progress: CountryProgress, info: Country?, onClick: () -> Unit) {
    val colors = LocalAppColors.current
    Row(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(10.dp))
            .clickable(onClick = onClick)
            .padding(8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(info?.flag.orEmpty(), fontSize = 20.sp, modifier = Modifier.width(36.dp))
        Text(info?.name ?: progress.code, color = colors.text, modifier = Modifier.weight(1f))
        Text("${progress.visited}/${progress.places}", fontSize = 13.sp, color = colors.muted)
        Box(Modifier.padding(start = 10.dp).size(10.dp).clip(CircleShape).background(statusColor(progress.status)))
    }
}

@Composable
fun CountryHeader(progress: CountryProgress, info: Country?, onBack: () -> Unit) {
    val colors = LocalAppColors.current
    Column(Modifier.padding(top = 4.dp, bottom = 8.dp)) {
        TextButton(onClick = onBack) { Text("← All countries", color = colors.muted) }
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(info?.flag.orEmpty(), fontSize = 44.sp)
            Column(Modifier.padding(start = 14.dp)) {
                Text(info?.name ?: progress.code, style = MaterialTheme.typography.headlineSmall, color = colors.text)
                Text(
                    if (progress.status == Status.COMPLETED) {
                        "Completed · all ${progress.places} visited"
                    } else {
                        "${progress.visited} of ${progress.places} ${if (progress.places == 1) "place" else "places"} visited"
                    },
                    fontSize = 13.sp,
                    color = colors.muted,
                )
            }
        }
        Bar(
            fraction(progress.visited, progress.places),
            Modifier.fillMaxWidth().padding(top = 10.dp),
            color = if (progress.status == Status.AVAILABLE) colors.accent else statusColor(progress.status),
        )
    }
}

private fun distance(meters: Int) = if (meters < 1000) "${meters / 10 * 10} m" else "%.1f km".format(meters / 1000.0)

@Composable
fun PlaceRow(place: Place, busy: Boolean, onToggle: () -> Unit) {
    val colors = LocalAppColors.current
    val context = LocalContext.current
    val open = { url: String -> context.startActivity(Intent(Intent.ACTION_VIEW, url.toUri())) }
    Row(
        Modifier
            .fillMaxWidth()
            .padding(vertical = 4.dp)
            .clip(RoundedCornerShape(12.dp))
            .background(colors.panelRaised)
            .border(
                1.dp,
                if (place.visited) colors.completed.copy(alpha = 0.45f) else Color.Transparent,
                RoundedCornerShape(12.dp),
            )
            .padding(12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(place.name, fontWeight = FontWeight.Bold, color = colors.text)
            Text(
                distance(place.distanceM) + (place.address?.let { " · $it" } ?: ""),
                fontSize = 13.sp,
                color = colors.muted,
            )
            Row(horizontalArrangement = Arrangement.spacedBy(14.dp), modifier = Modifier.padding(top = 4.dp)) {
                Text(
                    "Directions",
                    fontSize = 13.sp,
                    color = colors.accent,
                    modifier = Modifier.clickable {
                        open("https://www.google.com/maps/dir/?api=1&destination=${place.lat},${place.lng}")
                    },
                )
                place.url?.let { url ->
                    Text("Website", fontSize = 13.sp, color = colors.accent, modifier = Modifier.clickable { open(url) })
                }
                if (place.verified) Text("✓ verified", fontSize = 13.sp, color = colors.completed)
            }
        }
        if (place.visited) {
            Button(
                onClick = onToggle,
                enabled = !busy,
                colors = ButtonDefaults.buttonColors(containerColor = colors.completed, contentColor = colors.onCompleted),
            ) { Text("✓ Visited") }
        } else {
            OutlinedButton(onClick = onToggle, enabled = !busy, border = BorderStroke(1.dp, colors.accent)) {
                Text("Check in")
            }
        }
    }
}

@Composable
fun BadgeRow(badge: Badge) {
    val colors = LocalAppColors.current
    Row(
        Modifier
            .fillMaxWidth()
            .padding(vertical = 4.dp)
            .clip(RoundedCornerShape(12.dp))
            .background(colors.panelRaised)
            .padding(12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(badge.icon, fontSize = 26.sp, modifier = Modifier.alpha(if (badge.earned) 1f else 0.45f))
        Column(Modifier.padding(start = 12.dp).weight(1f)) {
            Text(badge.title, fontWeight = FontWeight.Bold, color = colors.text)
            Text(badge.description, fontSize = 13.sp, color = colors.muted)
            if (!badge.earned) {
                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 6.dp)) {
                    Bar(fraction(badge.progress, badge.target), Modifier.weight(1f))
                    Text("  ${badge.progress}/${badge.target}", fontSize = 12.sp, color = colors.muted)
                }
            }
        }
        if (badge.earned) Text("✓", color = colors.completed, fontWeight = FontWeight.ExtraBold)
    }
}
