package app.mangiamundo.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.rounded.Map
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import app.mangiamundo.BuildConfig

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
        Text(
            "Privacy policy",
            color = colors.accent,
            fontWeight = FontWeight.Bold,
            modifier = Modifier.padding(top = 12.dp).clickable { context.open("https://mangiamundo.com/privacy/") },
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
