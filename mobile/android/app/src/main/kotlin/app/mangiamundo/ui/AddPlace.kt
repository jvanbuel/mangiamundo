package app.mangiamundo.ui

import android.net.Uri
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import app.mangiamundo.core.Country
import androidx.compose.runtime.LaunchedEffect

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
