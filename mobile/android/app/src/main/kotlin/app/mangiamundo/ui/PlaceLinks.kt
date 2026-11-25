package app.mangiamundo.ui

import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.core.net.toUri
import app.mangiamundo.core.Place

/** geo: opens whichever maps app the user has; the web map covers phones without one. */
private fun android.content.Context.directions(place: Place) =
    try {
        startActivity(Intent(Intent.ACTION_VIEW, "geo:0,0?q=${place.lat},${place.lng}(${Uri.encode(place.name)})".toUri()))
    } catch (_: ActivityNotFoundException) {
        open("https://www.openstreetmap.org/directions?route=%3B${place.lat}%2C${place.lng}")
    }

/** A maps app with an easy OpenStreetMap editor, opened by "Something wrong?" instead of openstreetmap.org. */
internal enum class EditorApp(val scheme: String, val label: String) {
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
internal fun android.content.Context.editorApp(): EditorApp? = EditorApp.entries.firstOrNull { app ->
    // Screenshot tests have no real package manager.
    runCatching { Intent(Intent.ACTION_VIEW, "${app.scheme}://map".toUri()).resolveActivity(packageManager) != null }
        .getOrDefault(false)
}

/** Preserve the editor deep link, but use the OSM page if the editor becomes unavailable. */
internal fun android.content.Context.edit(place: Place, editor: EditorApp?, url: String) {
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
internal fun PlaceLinks(place: Place) {
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
