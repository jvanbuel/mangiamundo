package app.mangiamundo.ui

import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
import androidx.core.net.toUri
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.text.selection.SelectionContainer
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
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.AddLocationAlt
import androidx.compose.material.icons.rounded.Celebration
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Share
import androidx.compose.material.icons.rounded.Diamond
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Notifications
import androidx.compose.material.icons.rounded.NotificationsActive
import androidx.compose.material.icons.rounded.EmojiEvents
import androidx.compose.material.icons.rounded.Flag
import androidx.compose.material.icons.rounded.Map
import androidx.compose.material.icons.rounded.NearMe
import androidx.compose.material.icons.rounded.Place
import androidx.compose.material.icons.rounded.Public
import androidx.compose.material.icons.rounded.Restaurant
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material.icons.rounded.Verified
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
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
import androidx.compose.runtime.DisposableEffect
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
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
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
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import app.mangiamundo.AppViewModel
import app.mangiamundo.DetailVisits
import java.time.LocalDate
import app.mangiamundo.BuildConfig
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
import app.mangiamundo.core.AreaProgress
import app.mangiamundo.core.GroupProgress
import app.mangiamundo.core.Place
import app.mangiamundo.core.Status
import kotlinx.coroutines.launch
import androidx.compose.foundation.layout.offset
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.viewinterop.AndroidView
import org.maplibre.android.MapLibre
import org.maplibre.android.camera.CameraPosition
import org.maplibre.android.geometry.LatLng as MapLatLng
import org.maplibre.android.maps.MapView

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
private fun Bar(fraction: Float, modifier: Modifier = Modifier, color: Color = LocalAppColors.current.accent) =
    Bar(fraction, modifier, SolidColor(color))

/** A progress bar; the empty part is outlined, so it shows even at zero. */
@Composable
private fun Bar(fraction: Float, modifier: Modifier = Modifier, brush: Brush) {
    val colors = LocalAppColors.current
    val shape = RoundedCornerShape(50)
    Box(modifier.height(6.dp).clip(shape).background(colors.track).border(1.dp, colors.trackEdge, shape)) {
        Box(Modifier.fillMaxHeight().fillMaxWidth(fraction.coerceIn(0f, 1f)).clip(shape).background(brush))
    }
}

private fun fraction(part: Int, whole: Int) = if (whole == 0) 0f else part.toFloat() / whole

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
fun Header(location: SavedLocation?, onChange: () -> Unit, onSettings: () -> Unit) {
    val colors = LocalAppColors.current
    Column(Modifier.padding(bottom = 6.dp)) {
        Row(verticalAlignment = Alignment.Top) {
            Column(Modifier.weight(1f)) {
                Text("Mangiamundo", style = MaterialTheme.typography.titleLarge, color = colors.text)
                Text("Eat the world, start with your city", style = MaterialTheme.typography.titleSmall, color = colors.muted)
            }
            IconButton(onClick = onSettings) {
                Icon(Icons.Rounded.Settings, contentDescription = "Settings", tint = colors.muted)
            }
        }
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

/** Rarely needed things, kept off the main screen. */
@Composable
fun SettingsPanel(server: String?, account: @Composable () -> Unit = {}) {
    val colors = LocalAppColors.current
    val context = LocalContext.current
    Column {
        Text("Settings", style = MaterialTheme.typography.titleLarge, color = colors.text)
        account()
        Text(
            "Map data © OpenStreetMap contributors",
            color = colors.accent,
            fontWeight = FontWeight.Bold,
            modifier = Modifier.padding(top = 16.dp).clickable { context.open("https://www.openstreetmap.org/copyright") },
        )
        BuildLine(server)
    }
}

/** Which builds of the app and the server are running, to tell old builds apart. */
@Composable
private fun BuildLine(server: String?) {
    val app = "${BuildConfig.VERSION_NAME} (${BuildConfig.VERSION_CODE}, ${BuildConfig.GIT_COMMIT})"
    SelectionContainer {
        Text(
            "App $app" + server?.let { " · Server $it" }.orEmpty(),
            color = LocalAppColors.current.muted,
            style = MaterialTheme.typography.labelSmall,
            modifier = Modifier.padding(top = 16.dp),
        )
    }
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

        // Recent areas while nothing is typed; search results once something is.
        val recent = vm.recentAreas.filter { it.id != vm.location?.area?.id }.take(5)
        if (query.isBlank() && recent.isNotEmpty()) {
            Text("Recent", style = MaterialTheme.typography.titleSmall, color = colors.muted)
            for (area in recent) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    AreaRow(area, Modifier.weight(1f)) {
                        vm.chooseArea(area)
                        onDone()
                    }
                    IconButton(onClick = { vm.forgetRecent(area.id) }) {
                        Icon(Icons.Rounded.Close, contentDescription = "Remove ${area.name} from recent", tint = colors.muted)
                    }
                }
            }
        }

        results?.takeIf { query.isNotBlank() }?.let { found ->
            if (found.isEmpty()) {
                Text("No city or postal code found. Try a different spelling.", color = colors.muted, fontSize = 13.sp)
            }
            for (result in found) {
                AreaRow(result, Modifier.fillMaxWidth()) {
                    vm.chooseArea(result)
                    onDone()
                }
            }
        }
    }
}

/** A city or postal code in the picker: its name, then its kind and where it is. */
@Composable
private fun AreaRow(area: Area, modifier: Modifier, onClick: () -> Unit) {
    val colors = LocalAppColors.current
    Column(modifier.clip(RoundedCornerShape(8.dp)).clickable(onClick = onClick).padding(10.dp)) {
        Text(area.name, color = colors.text)
        val kind = if (area.kind == AreaKind.CITY) "City" else "Postal code"
        Text(listOfNotNull(kind, area.detail).joinToString(" · "), color = colors.muted, fontSize = 12.sp)
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
fun Passport(discover: Discover, onShare: (() -> Unit)? = null) {
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
            }
            if (onShare != null && discover.placesVisited > 0) {
                ChunkyButton("Share my map", onClick = onShare, icon = Icons.Rounded.Share)
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
        Box(Modifier.padding(start = 10.dp)) { StatusMark(progress.status) }
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

/** Opens a URL in the browser or the app that handles it, or says there is none. */
private fun android.content.Context.open(url: String) =
    try {
        startActivity(Intent(Intent.ACTION_VIEW, url.toUri()))
    } catch (_: ActivityNotFoundException) {
        android.widget.Toast.makeText(this, "No app on this phone can open that link.", android.widget.Toast.LENGTH_SHORT).show()
    }

/** geo: opens whichever maps app the user has; the web map covers phones without one. */
private fun android.content.Context.directions(place: Place) =
    try {
        startActivity(Intent(Intent.ACTION_VIEW, "geo:0,0?q=${place.lat},${place.lng}(${Uri.encode(place.name)})".toUri()))
    } catch (_: ActivityNotFoundException) {
        open("https://www.openstreetmap.org/directions?route=%3B${place.lat}%2C${place.lng}")
    }

/** A maps app with an easy OpenStreetMap editor, opened by "Something wrong?" instead of openstreetmap.org. */
private enum class EditorApp(val scheme: String, val label: String) {
    COMAPS("cm", "CoMaps"),
    ORGANIC_MAPS("om", "Organic Maps");

    /** The app's place card at the place, which shows the OSM feature there with "Edit place". `ll` must come before `n`. */
    fun link(place: Place) = link(place.name, place.lat, place.lng)

    fun link(name: String, lat: Double, lng: Double) = "$scheme://map?ll=$lat,$lng&n=${Uri.encode(name)}"
}

/** Opens the editor app at a new place's pin, for the user to add it to OSM there. */
fun android.content.Context.addInEditorApp(name: String, lat: Double, lng: Double) {
    val editor = editorApp() ?: return
    runCatching { startActivity(Intent(Intent.ACTION_VIEW, editor.link(name, lat, lng).toUri())) }
}

/** The first editor app installed, if any. Needs the schemes in `<queries>` (AndroidManifest.xml). */
private fun android.content.Context.editorApp(): EditorApp? = EditorApp.entries.firstOrNull { app ->
    // Screenshot tests have no real package manager.
    runCatching { Intent(Intent.ACTION_VIEW, "${app.scheme}://map".toUri()).resolveActivity(packageManager) != null }
        .getOrDefault(false)
}

/** Preserve the editor deep link, but use the OSM page if the editor becomes unavailable. */
private fun android.content.Context.edit(place: Place, editor: EditorApp?, url: String) {
    if (editor == null) {
        open(url)
        return
    }
    try {
        startActivity(Intent(Intent.ACTION_VIEW, editor.link(place).toUri()))
    } catch (_: ActivityNotFoundException) {
        open(url)
    } catch (_: SecurityException) {
        open(url)
    }
}

@Composable
private fun PlaceLinks(place: Place) {
    val colors = LocalAppColors.current
    val context = LocalContext.current
    Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        Text(
            "Directions",
            style = MaterialTheme.typography.labelLarge,
            color = colors.accent,
            modifier = Modifier.clickable { context.directions(place) },
        )
        place.url?.let { url ->
            Text("Website", style = MaterialTheme.typography.labelLarge, color = colors.accent, modifier = Modifier.clickable { context.open(url) })
        }
    }
}

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
            Text(
                date + if (visit.verified) " · checked in on the spot" else "",
                style = body,
                color = colors.text,
                modifier = Modifier.padding(vertical = 6.dp),
            )
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

/** A city's restaurants loading: the percentage in a ring, with what the server is doing below. */
@Composable
fun LoadingWheel(progress: AreaProgress, modifier: Modifier = Modifier) {
    val colors = LocalAppColors.current
    val animated by animateFloatAsState(progress.percent / 100f, label = "area progress")
    Column(
        modifier
            .clip(RoundedCornerShape(14.dp))
            .background(colors.panel)
            .padding(20.dp)
            .semantics(mergeDescendants = true) { contentDescription = "${progress.text}, ${progress.percent} percent" },
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Box(contentAlignment = Alignment.Center) {
            CircularProgressIndicator(
                progress = { animated },
                modifier = Modifier.size(72.dp),
                color = colors.accent,
                strokeWidth = 6.dp,
                trackColor = colors.line,
            )
            Text("${progress.percent}%", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.ExtraBold, color = colors.text)
        }
        Text(progress.text, style = MaterialTheme.typography.bodyMedium, color = colors.muted, modifier = Modifier.padding(top = 10.dp))
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

private fun BadgeKind.icon(): ImageVector = when (this) {
    BadgeKind.VISITS -> Icons.Rounded.Restaurant
    BadgeKind.COUNTRIES -> Icons.Rounded.Flag
    BadgeKind.COMPLETED -> Icons.Rounded.Verified
    BadgeKind.CONTINENT -> Icons.Rounded.Map
    BadgeKind.WORLD -> Icons.Rounded.Public
    BadgeKind.PIONEER -> Icons.Rounded.AddLocationAlt
    BadgeKind.EVERYTHING, BadgeKind.UNKNOWN -> Icons.Rounded.EmojiEvents
}

/** Gold for tasting, diamond for finishing, blue for the rest. */
enum class MedalFace { GOLD, DIAMOND, BLUE, EVENT }

private fun BadgeKind.face(): MedalFace = when (this) {
    BadgeKind.VISITS, BadgeKind.COUNTRIES -> MedalFace.GOLD
    BadgeKind.COMPLETED, BadgeKind.CONTINENT, BadgeKind.WORLD, BadgeKind.EVERYTHING -> MedalFace.DIAMOND
    BadgeKind.PIONEER, BadgeKind.UNKNOWN -> MedalFace.BLUE
}

/** A solid token with a darker lower edge, so it reads as something you collect. */
@Composable
private fun MedalFace(icon: ImageVector, face: MedalFace, earned: Boolean, size: Dp) {
    val colors = LocalAppColors.current
    val fill: Brush = when {
        !earned -> SolidColor(colors.line)
        face == MedalFace.GOLD -> SolidColor(colors.discovered)
        face == MedalFace.DIAMOND -> colors.diamond
        face == MedalFace.EVENT -> SolidColor(colors.event)
        else -> SolidColor(colors.accent)
    }
    val edge = when {
        !earned -> lerp(colors.line, Color.Black, 0.3f)
        face == MedalFace.GOLD -> colors.discoveredEdge
        face == MedalFace.DIAMOND -> colors.completedEdge
        face == MedalFace.EVENT -> lerp(colors.event, Color.Black, 0.3f)
        else -> lerp(colors.accent, Color.Black, 0.3f)
    }
    val ink = when {
        !earned -> colors.muted
        face == MedalFace.GOLD -> colors.onDiscovered
        face == MedalFace.DIAMOND -> colors.onCompleted
        else -> colors.onAccent
    }
    Box(
        Modifier
            .size(size)
            .clip(CircleShape)
            .background(edge)
            .padding(bottom = size * 0.07f)
            .clip(CircleShape)
            .background(fill),
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, contentDescription = null, tint = ink, modifier = Modifier.size(size * 0.46f))
    }
}

@Composable
fun Medal(kind: BadgeKind, earned: Boolean, size: Dp = 64.dp) = MedalFace(kind.icon(), kind.face(), earned, size)

/** The medal of a limited-time event. */
@Composable
fun EventMedal(earned: Boolean, size: Dp = 64.dp) = MedalFace(Icons.Rounded.Celebration, MedalFace.EVENT, earned, size)

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
            color = colors.onAccent,
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

private class Tile(val key: String, val title: String, val detail: String, val medal: @Composable () -> Unit)

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun BadgeGrid(badges: List<Badge>, events: List<EventProgress> = emptyList(), today: java.time.LocalDate = java.time.LocalDate.now()) {
    val colors = LocalAppColors.current
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

/** "Add a missing restaurant": it goes on the map where the user stands. */
@Composable
fun AddPlaceForm(
    countries: List<Country>,
    busy: Boolean,
    startingPoint: suspend () -> Pair<Double, Double>,
    onCancel: () -> Unit,
    onAdd: (name: String, country: String, lat: Double, lng: Double, address: String?, website: String?, note: Boolean) -> Unit,
) {
    val colors = LocalAppColors.current
    val context = LocalContext.current
    var name by rememberSaveable { mutableStateOf("") }
    var country by rememberSaveable { mutableStateOf<String?>(null) }
    var address by rememberSaveable { mutableStateOf("") }
    var website by rememberSaveable { mutableStateOf("") }
    var choosing by remember { mutableStateOf(false) }
    // Where the map starts, and the point under the pin as the map moves.
    // The point is saved (Pair is Serializable), so a rotation keeps the pin where the user put it.
    var point by rememberSaveable { mutableStateOf<Pair<Double, Double>?>(null) }
    var start by remember { mutableStateOf(point) }
    LaunchedEffect(Unit) { if (start == null) startingPoint().let { start = it; point = it } }
    // Only countries with a cuisine can be the food a place serves.
    val choices = remember(countries) { countries.filter { it.cuisine != null }.sortedBy { it.name } }
    val chosen = choices.firstOrNull { it.code == country }
    // A maps app the user can add the place to OSM with themselves.
    val editor = remember { context.editorApp() }
    val ready = !busy && point != null && name.isNotBlank() && chosen != null
    val submit = { note: Boolean ->
        point?.let { (lat, lng) ->
            onAdd(name.trim(), country.orEmpty(), lat, lng, address.trim().ifEmpty { null }, website.trim().ifEmpty { null }, note)
        }
    }

    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Text("Add a missing restaurant", style = MaterialTheme.typography.titleLarge, color = colors.text)
        OutlinedTextField(name, { name = it }, label = { Text("Name") }, singleLine = true, modifier = Modifier.fillMaxWidth())
        Box {
            OutlinedButton(onClick = { choosing = true }, modifier = Modifier.fillMaxWidth()) {
                Text(chosen?.let { "Food from ${it.flag} ${it.name}" } ?: "Food from… choose a country")
            }
            DropdownMenu(expanded = choosing, onDismissRequest = { choosing = false }) {
                for (c in choices) {
                    DropdownMenuItem(text = { Text("${c.flag} ${c.name}") }, onClick = { country = c.code; choosing = false })
                }
            }
        }
        Text("Where is it?", color = colors.text, fontWeight = FontWeight.Bold)
        start?.let { (lat, lng) ->
            PinMap(lat, lng, onMove = { a, b -> point = a to b }, modifier = Modifier.fillMaxWidth().height(220.dp).clip(RoundedCornerShape(12.dp)))
            Text("Drag the map until the pin is on the restaurant's door.", style = MaterialTheme.typography.bodySmall, color = colors.muted)
        } ?: Text("Finding your position…", color = colors.muted)
        OutlinedTextField(address, { address = it }, label = { Text("Address (optional)") }, singleLine = true, modifier = Modifier.fillMaxWidth())
        OutlinedTextField(
            website,
            { website = it },
            label = { Text("Website (optional)") },
            placeholder = { Text("https://") },
            singleLine = true,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
            modifier = Modifier.fillMaxWidth(),
        )
        Text(
            "Only you see it until it is on OpenStreetMap. We ask its mappers to add it by posting a note, shared under the Open Database License.",
            style = MaterialTheme.typography.bodySmall,
            color = colors.muted,
            modifier = Modifier.clickable { context.open("https://www.openstreetmap.org/copyright") },
        )
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
            TextButton(onClick = onCancel, enabled = !busy) { Text("Cancel") }
            Button(
                onClick = { submit(true) },
                enabled = ready,
            ) { Text(if (busy) "Adding…" else "Add it") }
        }
        editor?.let { app ->
            OutlinedButton(onClick = { submit(false) }, enabled = ready, modifier = Modifier.fillMaxWidth()) {
                Text("Add it yourself in ${app.label}")
            }
            Text(
                "Saves it here, then opens ${app.label} at the pin: add it there with \"Add a place to the map\", and it's on OpenStreetMap right away. No note is sent.",
                style = MaterialTheme.typography.bodySmall,
                color = colors.muted,
            )
        }
    }
}

/** A street map with a pin fixed in its middle; `onMove` gets the point under the pin. */
@Composable
private fun PinMap(lat: Double, lng: Double, onMove: (Double, Double) -> Unit, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    val moved by rememberUpdatedState(onMove)
    val mapView = remember {
        MapLibre.getInstance(context)
        MapView(context).apply {
            onCreate(null)
            // Drags move the map, not the scrolling sheet around it.
            setOnTouchListener { view, _ ->
                view.parent?.requestDisallowInterceptTouchEvent(true)
                false
            }
            getMapAsync { map ->
                // Free street tiles from OpenStreetMap data, no key needed.
                map.setStyle("https://tiles.openfreemap.org/styles/liberty")
                map.cameraPosition = CameraPosition.Builder().target(MapLatLng(lat, lng)).zoom(17.0).build()
                map.uiSettings.isRotateGesturesEnabled = false
                map.uiSettings.isTiltGesturesEnabled = false
                map.addOnCameraMoveListener { map.cameraPosition.target?.let { moved(it.latitude, it.longitude) } }
            }
        }
    }
    // MapView follows the screen's lifecycle; adding the observer replays the events so far.
    DisposableEffect(lifecycle) {
        val observer = LifecycleEventObserver { _, event ->
            when (event) {
                Lifecycle.Event.ON_START -> mapView.onStart()
                Lifecycle.Event.ON_RESUME -> mapView.onResume()
                Lifecycle.Event.ON_PAUSE -> mapView.onPause()
                Lifecycle.Event.ON_STOP -> mapView.onStop()
                else -> Unit
            }
        }
        lifecycle.addObserver(observer)
        onDispose {
            lifecycle.removeObserver(observer)
            mapView.onPause()
            mapView.onStop()
            mapView.onDestroy()
        }
    }
    Box(modifier) {
        AndroidView({ mapView }, Modifier.fillMaxWidth().fillMaxHeight())
        // The pin's tip marks the map's centre.
        Icon(
            Icons.Rounded.Place,
            contentDescription = null,
            tint = LocalAppColors.current.accent,
            modifier = Modifier.align(Alignment.Center).offset(y = (-17).dp).size(34.dp),
        )
    }
}
