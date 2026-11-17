package app.mangiamundo.ui

import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
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
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.Celebration
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Notifications
import androidx.compose.material.icons.rounded.NotificationsActive
import androidx.compose.material.icons.rounded.EmojiEvents
import androidx.compose.material.icons.rounded.Flag
import androidx.compose.material.icons.rounded.Map
import androidx.compose.material.icons.rounded.NearMe
import androidx.compose.material.icons.rounded.Place
import androidx.compose.material.icons.rounded.Public
import androidx.compose.material.icons.rounded.Restaurant
import androidx.compose.material.icons.rounded.Verified
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
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
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
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
import app.mangiamundo.core.BadgeKind
import app.mangiamundo.core.Celebration
import app.mangiamundo.core.EventProgress
import app.mangiamundo.core.EventStatus
import app.mangiamundo.core.Country
import app.mangiamundo.core.CountryProgress
import app.mangiamundo.core.Discover
import app.mangiamundo.core.Area
import app.mangiamundo.core.AreaKind
import app.mangiamundo.core.GroupProgress
import app.mangiamundo.core.Place
import app.mangiamundo.core.Status
import kotlinx.coroutines.launch

private val CardShape = RoundedCornerShape(14.dp)

/** A panel with a solid outline and a heavier lower edge, rather than a soft tinted card. */
@Composable
private fun Card(modifier: Modifier = Modifier, content: @Composable () -> Unit) {
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
fun Header(location: SavedLocation?, onChange: () -> Unit) {
    val colors = LocalAppColors.current
    Column(Modifier.padding(bottom = 6.dp)) {
        Text("Mangiamundo", style = MaterialTheme.typography.titleLarge, color = colors.text)
        Text("Eat the world, start with your city", style = MaterialTheme.typography.titleSmall, color = colors.muted)
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
                Icon(
                    if (location.fromGps) Icons.Rounded.NearMe else Icons.Rounded.Place,
                    contentDescription = null,
                    tint = colors.text,
                    modifier = Modifier.size(16.dp).padding(end = 2.dp),
                )
                Text(
                    location.area.name,
                    fontWeight = FontWeight.Bold,
                    color = colors.text,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f, fill = false),
                )
                if (location.area.kind == AreaKind.POSTAL_CODE) Text("  ·  postal code", color = colors.muted)
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
                "Which of the world's kitchens are near you?",
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

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LocationPicker(vm: AppViewModel, onUseGps: () -> Unit, onDone: () -> Unit) {
    val colors = LocalAppColors.current
    val scope = rememberCoroutineScope()
    var query by rememberSaveable { mutableStateOf("") }
    var results by remember { mutableStateOf<List<Area>?>(null) }
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
        // The city and postal code found at the GPS position.
        val location = vm.location
        if (location != null && location.options.size > 1) {
            SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
                location.options.forEachIndexed { index, option ->
                    SegmentedButton(
                        selected = location.area.id == option.id,
                        onClick = {
                            vm.switchArea(option)
                            onDone()
                        },
                        shape = SegmentedButtonDefaults.itemShape(index, location.options.size),
                        icon = {},
                    ) {
                        Text(
                            if (option.kind == AreaKind.CITY) "All of ${option.name}" else "Only ${option.name}",
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
            }
        }
        ChunkyButton(
            if (vm.locating) "Finding you…" else "Use my current location",
            onClick = {
                onUseGps()
                onDone()
            },
            enabled = !vm.locating,
            icon = Icons.Rounded.NearMe,
            modifier = Modifier.fillMaxWidth(),
        )
        vm.locateError?.let { Text(it, color = colors.danger, fontSize = 13.sp) }

        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedTextField(
                value = query,
                onValueChange = { query = it },
                placeholder = { Text("City or postal code") },
                singleLine = true,
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                keyboardActions = KeyboardActions(onSearch = { search() }),
                modifier = Modifier.weight(1f),
            )
            OutlinedButton(onClick = { search() }, enabled = !searching) { Text(if (searching) "…" else "Search") }
        }
        failure?.let { Text(it, color = colors.danger, fontSize = 13.sp) }

        results?.let { found ->
            if (found.isEmpty()) {
                Text("No city or postal code found. Try a different spelling.", color = colors.muted, fontSize = 13.sp)
            }
            for (result in found) {
                Column(
                    Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(8.dp))
                        .clickable {
                            vm.chooseArea(result)
                            onDone()
                        }
                        .padding(10.dp),
                ) {
                    Text(result.name, color = colors.text)
                    val kind = if (result.kind == AreaKind.CITY) "City" else "Postal code"
                    Text(listOfNotNull(kind, result.detail).joinToString(" · "), color = colors.muted, fontSize = 12.sp)
                }
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
        TextButton(onClick = onBack) {
            Icon(Icons.AutoMirrored.Rounded.ArrowBack, contentDescription = null, tint = colors.muted, modifier = Modifier.size(18.dp))
            Text(" All countries", color = colors.muted, style = MaterialTheme.typography.labelLarge)
        }
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

@Composable
fun PlaceRow(place: Place, busy: Boolean, onToggle: () -> Unit) {
    val colors = LocalAppColors.current
    val context = LocalContext.current
    val open = { url: String -> context.startActivity(Intent(Intent.ACTION_VIEW, url.toUri())) }
    Column {
        Row(Modifier.fillMaxWidth().padding(vertical = 14.dp), verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f).padding(end = 12.dp)) {
                Text(place.name, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.ExtraBold, color = colors.text)
                place.hoursText?.let {
                    Text(
                        it,
                        style = MaterialTheme.typography.bodyMedium,
                        fontWeight = FontWeight.Bold,
                        color = if (place.openNow == true) colors.completed else colors.muted,
                    )
                }
                Text(
                    distance(place.distanceM) + (place.address?.let { " · $it" } ?: "") +
                        if (place.verified) " · checked in on the spot" else "",
                    style = MaterialTheme.typography.bodyMedium,
                    color = colors.muted,
                )
                Row(horizontalArrangement = Arrangement.spacedBy(16.dp), modifier = Modifier.padding(top = 4.dp)) {
                    Text(
                        "Directions",
                        style = MaterialTheme.typography.labelLarge,
                        color = colors.accent,
                        modifier = Modifier.clickable {
                            // geo: opens whichever maps app the user has; the web map covers phones without one.
                            try {
                                open("geo:0,0?q=${place.lat},${place.lng}(${Uri.encode(place.name)})")
                            } catch (_: ActivityNotFoundException) {
                                open("https://www.openstreetmap.org/directions?route=%3B${place.lat}%2C${place.lng}")
                            }
                        },
                    )
                    place.url?.let { url ->
                        Text("Website", style = MaterialTheme.typography.labelLarge, color = colors.accent, modifier = Modifier.clickable { open(url) })
                    }
                }
            }
            if (place.visited) {
                ChunkyButton("Visited", onToggle, face = colors.completed, content = colors.onCompleted, enabled = !busy, icon = Icons.Rounded.Check)
            } else {
                ChunkyButton("Check in", onToggle, enabled = !busy)
            }
        }
        HorizontalDivider(color = colors.line)
    }
}

private fun BadgeKind.icon(): ImageVector = when (this) {
    BadgeKind.VISITS -> Icons.Rounded.Restaurant
    BadgeKind.COUNTRIES -> Icons.Rounded.Flag
    BadgeKind.COMPLETED -> Icons.Rounded.Verified
    BadgeKind.CONTINENT -> Icons.Rounded.Map
    BadgeKind.WORLD -> Icons.Rounded.Public
    BadgeKind.EVERYTHING -> Icons.Rounded.EmojiEvents
}

@Composable
private fun BadgeKind.color(): Color = with(LocalAppColors.current) {
    when (this@color) {
        BadgeKind.VISITS, BadgeKind.COUNTRIES -> available
        BadgeKind.COMPLETED, BadgeKind.WORLD -> completed
        BadgeKind.CONTINENT, BadgeKind.EVERYTHING -> discovered
    }
}

/** A solid token with a darker lower edge, so it reads as something you collect. */
@Composable
private fun MedalFace(icon: ImageVector, color: Color, earned: Boolean, size: Dp) {
    val colors = LocalAppColors.current
    val face = if (earned) color else colors.line
    Box(
        Modifier
            .size(size)
            .clip(CircleShape)
            .background(lerp(face, Color.Black, 0.3f))
            .padding(bottom = size * 0.07f)
            .clip(CircleShape)
            .background(face),
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, contentDescription = null, tint = if (earned) Color.White else colors.muted, modifier = Modifier.size(size * 0.46f))
    }
}

@Composable
fun Medal(kind: BadgeKind, earned: Boolean, size: Dp = 64.dp) = MedalFace(kind.icon(), kind.color(), earned, size)

/** The medal of a limited-time event. */
@Composable
fun EventMedal(earned: Boolean, size: Dp = 64.dp) = MedalFace(Icons.Rounded.Celebration, LocalAppColors.current.event, earned, size)

private fun parseDay(day: String) = java.time.LocalDate.parse(day)
private val shortDate = java.time.format.DateTimeFormatter.ofPattern("d MMM")

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
            color = Color.White,
            textAlign = TextAlign.Center,
            modifier = Modifier.fillMaxWidth().background(colors.event).padding(vertical = 1.dp),
        )
        Text("${day.dayOfMonth}", style = MaterialTheme.typography.titleLarge, color = colors.text)
    }
}

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
                            color = colors.event,
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

private class Tile(val key: String, val title: String, val detail: String, val medal: @Composable () -> Unit)

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun BadgeGrid(badges: List<Badge>, events: List<EventProgress> = emptyList(), today: java.time.LocalDate = java.time.LocalDate.now()) {
    val colors = LocalAppColors.current
    val running = events.filter { it.status != EventStatus.ENDED && !it.earned }
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
