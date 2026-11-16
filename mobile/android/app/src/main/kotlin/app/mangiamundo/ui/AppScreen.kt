package app.mangiamundo.ui

import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.isSystemInDarkTheme
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
import androidx.compose.material3.Text
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
import app.mangiamundo.core.ProjectionKind

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AppScreen(vm: AppViewModel = viewModel()) {
    val colors = LocalAppColors.current
    val context = LocalContext.current
    val snackbar = remember { SnackbarHostState() }
    var pickerOpen by rememberSaveable { mutableStateOf(false) }
    var shareOpen by rememberSaveable { mutableStateOf(false) }
    var backupOpen by rememberSaveable { mutableStateOf(false) }
    val dark = isSystemInDarkTheme()
    // Ticks every minute so check-in cooldowns run out while the list is open.
    var now by remember { mutableStateOf(System.currentTimeMillis() / 1000) }
    LaunchedEffect(Unit) {
        while (true) {
            kotlinx.coroutines.delay(60_000)
            now = System.currentTimeMillis() / 1000
        }
    }

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
        snackbarHost = { SnackbarHost(snackbar) },
        containerColor = colors.panel,
        contentWindowInsets = WindowInsets(0, 0, 0, 0),
    ) { padding ->
        Column(Modifier.fillMaxSize().padding(padding)) {
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
                item { Header(vm.location, vm.radiusKm, onChange = { pickerOpen = true }) }

                when {
                    vm.location == null -> item {
                        Welcome { LocationPicker(vm, useGps, onDone = {}) }
                    }
                    vm.error != null -> item { ErrorCard(vm.error.orEmpty(), vm::refresh) }
                    discover == null -> item { Note("Scouting restaurants around ${vm.location?.name ?: "you"}…") }
                    discover.world.available == 0 -> item {
                        Note(
                            "No restaurants with a national cuisine turned up within ${vm.radiusKm} km. " +
                                "Try a larger radius or another location.",
                        )
                    }
                    else -> {
                        item { Passport(discover, onShare = { shareOpen = true }, onBackup = { backupOpen = true }) }
                        if (selected == null && vm.celebrations.isNotEmpty()) {
                            item { ComingUp(vm.celebrations, vm.remindersOn, toggleReminders, vm::country, onOpen = vm::select) }
                        }
                        if (selected != null) {
                            val tried = discover.dishesTried.toSet()
                            item {
                                CountryHeader(
                                    selected,
                                    vm.country(selected.code),
                                    dishes = vm.dishesOf(listOf(selected.code)),
                                    tried = tried,
                                    onBack = { vm.select(null) },
                                )
                            }
                            if (vm.placesLoading) item { Note("Loading places…") }
                            items(vm.places, key = { it.id }) { place ->
                                PlaceRow(
                                    place,
                                    busy = vm.busyPlace == place.id,
                                    dishes = vm.dishesOf(place.countries),
                                    tried = tried,
                                    nowSeconds = now,
                                    onDish = { vm.logDish(place, it) },
                                    onUndo = { vm.undoCheckIn(place) },
                                ) { vm.checkIn(place) }
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
                            } else {
                                item(key = "badges") { BadgeGrid(discover.badges, discover.events) }
                            }
                        }
                    }
                }
            }
        }
    }

    if (shareOpen) {
        ModalBottomSheet(onDismissRequest = { shareOpen = false }, containerColor = colors.panel) {
            Column(Modifier.padding(start = 16.dp, end = 16.dp, bottom = 24.dp)) { ShareSheet(vm, dark) }
        }
    }
    if (backupOpen) {
        ModalBottomSheet(onDismissRequest = { backupOpen = false }, containerColor = colors.panel) {
            Column(Modifier.padding(start = 16.dp, end = 16.dp, bottom = 24.dp)) {
                BackupSheet(vm, onDone = { backupOpen = false })
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
}
