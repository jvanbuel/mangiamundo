package app.mangiamundo.ui

import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.clickable
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Tab
import androidx.compose.material3.TabRow
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import app.mangiamundo.AppViewModel
import app.mangiamundo.DeviceLocation
import app.mangiamundo.Notifications
import android.Manifest
import android.os.Build
import app.mangiamundo.core.AreaKind
import app.mangiamundo.core.ProjectionKind

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AppScreen(vm: AppViewModel = viewModel()) {
    val colors = LocalAppColors.current
    val context = LocalContext.current
    val snackbar = remember { SnackbarHostState() }
    var pickerOpen by rememberSaveable { mutableStateOf(false) }
    var settingsOpen by rememberSaveable { mutableStateOf(false) }
    var addOpen by rememberSaveable { mutableStateOf(false) }

    LaunchedEffect(Unit) {
        vm.messages.collect { snackbar.showSnackbar(it) }
    }

    val permission = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestMultiplePermissions(),
    ) { granted ->
        if (granted.values.any { it }) vm.locate() else vm.locationDenied()
    }
    val notifications = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        vm.setReminders(granted)
    }
    val toggleReminders = {
        when {
            vm.remindersOn -> vm.setReminders(false)
            Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU && !Notifications.allowed(context) ->
                notifications.launch(Manifest.permission.POST_NOTIFICATIONS)
            else -> vm.setReminders(true)
        }
    }
    // The new place goes where the user stands, so the form needs location access first.
    val addPermission = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestMultiplePermissions(),
    ) { granted ->
        if (granted.values.any { it }) addOpen = true else vm.locationDenied()
    }
    val openAdd = {
        // A submission still running would close a reopened form when it finishes.
        when {
            vm.addingPlace -> Unit
            DeviceLocation.hasPermission(context) -> addOpen = true
            else -> addPermission.launch(DeviceLocation.PERMISSIONS)
        }
    }
    val addLink: @Composable () -> Unit = {
        Text(
            "Is a restaurant missing? Add it",
            style = MaterialTheme.typography.bodySmall,
            color = colors.muted,
            textAlign = TextAlign.Center,
            modifier = Modifier.fillMaxWidth().padding(top = 16.dp).clickable { openAdd() },
        )
    }
    val useGps = {
        if (DeviceLocation.hasPermission(context)) vm.locate() else permission.launch(DeviceLocation.PERMISSIONS)
    }
    // First launch: ask where the user is straight away.
    LaunchedEffect(Unit) {
        if (vm.location == null && !DeviceLocation.hasPermission(context)) {
            permission.launch(DeviceLocation.PERMISSIONS)
        }
    }

    BackHandler(enabled = vm.selected != null) { vm.select(null) }

    val discover = vm.discover
    val selected = discover?.countries?.firstOrNull { it.code == vm.selected }
    val statuses = remember(discover) { discover?.countries.orEmpty().associate { it.code to it.status } }

    Scaffold(
        // The place sheet covers this host, so it shows messages itself while open.
        snackbarHost = { if (vm.detail == null && !addOpen) SnackbarHost(snackbar) },
        containerColor = colors.panel,
        contentWindowInsets = WindowInsets(0, 0, 0, 0),
    ) { padding ->
        Box(Modifier.fillMaxSize().padding(padding)) {
            Column(Modifier.fillMaxSize()) {
                Box(Modifier.fillMaxWidth().weight(0.42f).background(colors.bg)) {
                    vm.world?.let { world ->
                        WorldMap(
                            world = world,
                            statuses = statuses,
                            selected = vm.selected,
                            here = vm.location?.let { it.lng to it.lat },
                            kind = vm.projection,
                            camera = vm.camera,
                            onCamera = { vm.camera = it },
                            onTap = { vm.tapCountry(it.code, it.name) },
                            modifier = Modifier.fillMaxSize(),
                        )
                    }
                    SingleChoiceSegmentedButtonRow(
                        Modifier
                            .align(Alignment.TopEnd)
                            .windowInsetsPadding(WindowInsets.statusBars)
                            .padding(8.dp),
                    ) {
                        val kinds = listOf(ProjectionKind.GLOBE to "Globe", ProjectionKind.FLAT to "Flat")
                        kinds.forEachIndexed { index, (kind, label) ->
                            SegmentedButton(
                                selected = vm.projection == kind,
                                onClick = { vm.changeProjection(kind) },
                                shape = SegmentedButtonDefaults.itemShape(index, kinds.size),
                                icon = {},
                            ) { Text(label) }
                        }
                    }
                    Legend(Modifier.align(Alignment.BottomStart).padding(8.dp))
                }

                LazyColumn(
                    Modifier.fillMaxWidth().weight(0.58f),
                    contentPadding = PaddingValues(16.dp),
                ) {
                    item { Header(vm.location, onChange = { pickerOpen = true }, onSettings = { settingsOpen = true }) }

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
                            item { Passport(discover) }
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
                                    item(key = "add") { addLink() }
                                } else {
                                    item(key = "badges") { BadgeGrid(discover.badges, discover.events) }
                                }
                            }
                        }
                    }
                }
            }
            // While a city's restaurants load (slow outside the Belgium copy), show how far it is.
            vm.areaProgress?.let { LoadingWheel(it, Modifier.align(Alignment.Center)) }
        }
    }

    vm.detail?.let { place ->
        ModalBottomSheet(onDismissRequest = vm::closePlace, containerColor = colors.panel) {
            Box {
                Column(Modifier.verticalScroll(rememberScrollState())) {
                    PlaceDetail(
                        place,
                        vm::country,
                        busy = vm.busyPlace != null,
                        visits = vm.detailVisits,
                        onRetry = { vm.openPlace(place) },
                        onReportClosed = { vm.reportClosed(place) },
                    ) {
                        vm.toggleVisit(place)
                    }
                }
                // At the top: a half-open sheet's bottom edge is off screen.
                SnackbarHost(snackbar, Modifier.align(Alignment.TopCenter))
            }
        }
    }

    if (pickerOpen) {
        ModalBottomSheet(onDismissRequest = { pickerOpen = false }, containerColor = colors.panel) {
            Row(Modifier.padding(start = 16.dp, end = 16.dp, bottom = 24.dp)) {
                LocationPicker(vm, useGps, onDone = { pickerOpen = false })
            }
        }
    }

    if (addOpen) {
        ModalBottomSheet(onDismissRequest = { addOpen = false }, containerColor = colors.panel) {
            Box {
                Column(Modifier.verticalScroll(rememberScrollState()).padding(start = 16.dp, end = 16.dp, bottom = 24.dp)) {
                    AddPlaceForm(
                        vm.catalog?.countries.orEmpty(),
                        vm.addingPlace,
                        startingPoint = vm::startingPoint,
                        onCancel = { addOpen = false },
                    ) { name, country, lat, lng, address, website, note ->
                        vm.addPlace(name, country, lat, lng, address, website, note) {
                            addOpen = false
                            if (!note) context.addInEditorApp(name, lat, lng)
                        }
                    }
                }
                SnackbarHost(snackbar, Modifier.align(Alignment.TopCenter))
            }
        }
    }

    if (settingsOpen) {
        ModalBottomSheet(onDismissRequest = { settingsOpen = false }, containerColor = colors.panel) {
            Box(Modifier.padding(start = 16.dp, end = 16.dp, bottom = 24.dp)) { SettingsPanel(vm.server) }
        }
    }
}
