package app.mangiamundo.ui

import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.NearMe
import androidx.compose.material.icons.rounded.Place
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.mangiamundo.SavedLocation
import app.mangiamundo.core.AreaKind
import app.mangiamundo.core.Place

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
