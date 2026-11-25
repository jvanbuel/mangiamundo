package app.mangiamundo.ui

import android.Manifest
import android.os.Build
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.viewmodel.compose.viewModel
import app.mangiamundo.AppViewModel
import app.mangiamundo.DeviceLocation
import app.mangiamundo.Notifications

@Composable
fun AppScreen(vm: AppViewModel = viewModel()) {
    val colors = LocalAppColors.current
    val context = LocalContext.current
    val snackbar = remember { SnackbarHostState() }
    var pickerOpen by rememberSaveable { mutableStateOf(false) }
    var settingsOpen by rememberSaveable { mutableStateOf(false) }
    var addOpen by rememberSaveable { mutableStateOf(false) }
    var unknownOpen by rememberSaveable { mutableStateOf(false) }

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
    // The map starts at the user's position, so ask for location access first.
    val addPermission = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestMultiplePermissions(),
    ) { _ ->
        // Without access the map starts at the area's centre instead.
        addOpen = true
    }
    val openAdd = {
        // A submission still running would close a reopened form when it finishes.
        when {
            vm.addingPlace -> Unit
            DeviceLocation.hasPermission(context) -> addOpen = true
            else -> addPermission.launch(DeviceLocation.PERMISSIONS)
        }
    }
    // "Look again" needs the device's position too.
    val nearbyPermission = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestMultiplePermissions(),
    ) { _ ->
        if (DeviceLocation.hasPermission(context)) vm.lookAgainNearby()
    }
    val lookAgain = {
        if (DeviceLocation.hasPermission(context)) vm.lookAgainNearby() else nearbyPermission.launch(DeviceLocation.PERMISSIONS)
    }
    // Back from editing a place in an OpenStreetMap app: show the edit now.
    val lifecycleOwner = LocalLifecycleOwner.current
    DisposableEffect(lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) vm.onResume()
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }
    val addLink: @Composable () -> Unit = {
        Text(
            "Is a restaurant missing? Add it",
            style = MaterialTheme.typography.bodySmall,
            color = colors.muted,
            textAlign = TextAlign.Center,
            modifier = Modifier.fillMaxWidth().padding(top = 16.dp).clickable { openAdd() },
        )
        Text(
            if (vm.lookingNearby) "Looking…" else "Just added it in an OpenStreetMap app? Look again",
            style = MaterialTheme.typography.bodySmall,
            color = colors.muted,
            textAlign = TextAlign.Center,
            modifier = Modifier.fillMaxWidth().padding(top = 4.dp).clickable(enabled = !vm.lookingNearby) { lookAgain() },
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
    LaunchedEffect(vm.location?.area?.id) { vm.loadUnknownPlaces() }

    val discover = vm.discover
    val selected = discover?.countries?.firstOrNull { it.code == vm.selected }
    val statuses = remember(discover) { discover?.countries.orEmpty().associate { it.code to it.status } }

    Scaffold(
        // The place sheet covers this host, so it shows messages itself while open.
        snackbarHost = { if (vm.detail == null && !addOpen && !settingsOpen) SnackbarHost(snackbar) },
        containerColor = colors.panel,
        contentWindowInsets = WindowInsets(0, 0, 0, 0),
    ) { padding ->
        Box(Modifier.fillMaxSize().padding(padding)) {
            Column(Modifier.fillMaxSize()) {
                MapPane(vm, discover, statuses, Modifier.fillMaxWidth().weight(0.42f))

                HomeList(
                    vm,
                    discover,
                    selected,
                    useGps,
                    toggleReminders,
                    onChangeLocation = { pickerOpen = true },
                    onSettings = { settingsOpen = true },
                    onUnknown = { unknownOpen = true },
                    addLink,
                    Modifier.fillMaxWidth().weight(0.58f),
                )
            }
            // While a city's restaurants load (slow outside the Belgium copy), show how far it is.
            vm.areaProgress?.let { LoadingWheel(it, Modifier.align(Alignment.Center)) }
        }
    }

    vm.detail?.let { place -> PlaceDetailSheet(vm, place, snackbar) }
    if (pickerOpen) LocationSheet(vm, useGps, onClose = { pickerOpen = false })
    if (addOpen) AddPlaceSheet(vm, snackbar, onClose = { addOpen = false })
    if (unknownOpen) UnknownPlacesSheet(vm, snackbar, onClose = { unknownOpen = false })
    if (settingsOpen) SettingsSheet(vm, snackbar, onClose = { settingsOpen = false })
}
