package app.mangiamundo.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.NearMe
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.mangiamundo.AppViewModel
import app.mangiamundo.core.Area
import app.mangiamundo.core.AreaKind
import kotlinx.coroutines.launch

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
