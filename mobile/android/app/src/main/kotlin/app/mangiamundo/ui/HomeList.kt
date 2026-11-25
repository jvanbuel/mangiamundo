package app.mangiamundo.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Tab
import androidx.compose.material3.TabRow
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.mangiamundo.AppViewModel
import app.mangiamundo.core.AreaKind
import app.mangiamundo.core.CountryProgress
import app.mangiamundo.core.Discover

/**
 * The bottom of the home screen: the area, then the welcome, the passport and the countries
 * or badges, or a selected country's places.
 */
@Composable
internal fun HomeList(
    vm: AppViewModel,
    discover: Discover?,
    selected: CountryProgress?,
    useGps: () -> Unit,
    toggleReminders: () -> Unit,
    onChangeLocation: () -> Unit,
    onSettings: () -> Unit,
    onUnknown: () -> Unit,
    addLink: @Composable () -> Unit,
    modifier: Modifier,
) {
    val colors = LocalAppColors.current
    LazyColumn(
        modifier,
        contentPadding = PaddingValues(16.dp),
    ) {
        item { Header(vm.location, onChange = onChangeLocation, onSettings = onSettings) }

        when {
            vm.location == null -> item {
                Welcome { LocationPicker(vm, useGps, onDone = {}) }
            }
            vm.error != null -> item { ErrorCard(vm.error.orEmpty(), vm::refresh) }
            discover == null -> item { Note("Scouting restaurants in ${vm.location?.area?.name}…") }
            discover.world.available == 0 -> {
                item {
                    val postalCode = vm.location?.area?.kind == AreaKind.POSTAL_CODE
                    Note(
                        "No restaurants with a national cuisine turned up in ${vm.location?.area?.name}. " +
                            if (postalCode) "Try the whole city or another location." else "Try another location.",
                    )
                }
                item(key = "add") { addLink() }
            }
            else -> {
                item { Passport(discover, sharing = vm.sharing, onShare = vm::shareProgress) }
                if (selected == null && vm.celebrations.isNotEmpty()) {
                    item { ComingUp(vm.celebrations, vm.remindersOn, toggleReminders, vm::country, onOpen = vm::select) }
                }
                if (selected != null) {
                    item { CountryHeader(selected, vm.country(selected.code), onBack = { vm.select(null) }) }
                    if (vm.placesLoading) item { Note("Loading places…") }
                    // The API sorts open places first, then closed, then unknown hours.
                    val showGroups = vm.places.any { it.openNow != null }
                    for ((openNow, group) in vm.places.groupBy { it.openNow }) {
                        if (showGroups) item(key = "group-$openNow") { PlaceGroupHeader(openNow) }
                        items(group, key = { it.id }) { place ->
                            PlaceRow(place, busy = vm.busyPlace != null, onOpen = { vm.openPlace(place) }) { vm.toggleVisit(place) }
                        }
                    }
                } else {
                    item {
                        TabRow(selectedTabIndex = vm.tab, containerColor = colors.panel) {
                            Tab(vm.tab == 0, { vm.tab = 0 }, text = { Text("Countries ${discover.world.available}") })
                            val earned = discover.badges.count { it.earned }
                            Tab(vm.tab == 1, { vm.tab = 1 }, text = { Text("Badges $earned/${discover.badges.size}") })
                        }
                    }
                    if (vm.tab == 0) {
                        for (continent in discover.continents.filter { it.available > 0 }) {
                            item(key = "c-${continent.code}") { ContinentHeader(continent) }
                            val rows = discover.countries
                                .filter { it.continent == continent.code }
                                .sortedBy { vm.country(it.code)?.name ?: it.code }
                            items(rows, key = { it.code }) { progress ->
                                CountryRow(progress, vm.country(progress.code)) { vm.select(progress.code) }
                            }
                        }
                        if (vm.unknownPlaces.isNotEmpty()) item(key = "unknown") {
                            val n = vm.unknownPlaces.size
                            Box(Modifier.fillMaxWidth().padding(top = 16.dp), contentAlignment = Alignment.Center) {
                                ChunkyButton(
                                    if (n == 1) "1 restaurant doesn't say what food it serves" else "$n restaurants don't say what food they serve",
                                    onClick = onUnknown,
                                    face = colors.panelRaised,
                                    content = colors.text,
                                )
                            }
                        }
                        item(key = "add") { addLink() }
                    } else {
                        item(key = "badges") { BadgeGrid(discover.badges, discover.events, discover.quests) }
                    }
                }
            }
        }
    }
}
