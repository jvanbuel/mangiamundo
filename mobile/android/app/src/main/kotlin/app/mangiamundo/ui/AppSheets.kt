package app.mangiamundo.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import app.mangiamundo.AppViewModel
import app.mangiamundo.core.Place

// The sheets the home screen opens. Each covers the screen's snackbar host, so it shows
// messages itself.

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun PlaceDetailSheet(vm: AppViewModel, place: Place, snackbar: SnackbarHostState) {
    val colors = LocalAppColors.current
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
                    refreshing = vm.refreshingPlace == place.id,
                    onRefresh = { vm.refreshPlace(place.id) },
                    onEdit = { vm.editingOnOsm(place.id) },
                    onShareVisit = vm::shareVisit,
                    sharing = vm.sharing,
                    spun = vm.spun == place.id,
                    spinning = vm.spinning,
                    onSpin = vm::spin,
                    onNotToday = { vm.notToday(place.id) },
                ) {
                    vm.toggleVisit(place)
                }
            }
            // At the top: a half-open sheet's bottom edge is off screen.
            SnackbarHost(snackbar, Modifier.align(Alignment.TopCenter))
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun LocationSheet(vm: AppViewModel, useGps: () -> Unit, onClose: () -> Unit) {
    val colors = LocalAppColors.current
    ModalBottomSheet(onDismissRequest = onClose, containerColor = colors.panel) {
        Row(Modifier.padding(start = 16.dp, end = 16.dp, bottom = 24.dp)) {
            LocationPicker(vm, useGps, onDone = onClose)
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun AddPlaceSheet(vm: AppViewModel, snackbar: SnackbarHostState, onClose: () -> Unit) {
    val colors = LocalAppColors.current
    val context = LocalContext.current
    ModalBottomSheet(onDismissRequest = onClose, containerColor = colors.panel) {
        Box {
            Column(Modifier.verticalScroll(rememberScrollState()).padding(start = 16.dp, end = 16.dp, bottom = 24.dp)) {
                AddPlaceForm(
                    vm.catalog?.countries.orEmpty(),
                    vm.addingPlace,
                    startingPoint = vm::startingPoint,
                    onCancel = onClose,
                ) { name, country, lat, lng, address, website, note ->
                    vm.addPlace(name, country, lat, lng, address, website, note) {
                        onClose()
                        if (!note) context.addInEditorApp(name, lat, lng)
                    }
                }
            }
            SnackbarHost(snackbar, Modifier.align(Alignment.TopCenter))
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun UnknownPlacesSheet(vm: AppViewModel, snackbar: SnackbarHostState, onClose: () -> Unit) {
    val colors = LocalAppColors.current
    ModalBottomSheet(onDismissRequest = onClose, containerColor = colors.panel) {
        Box {
            Column(Modifier.verticalScroll(rememberScrollState()).padding(start = 16.dp, end = 16.dp, bottom = 24.dp)) {
                UnknownFoodSheet(vm.unknownPlaces, vm.catalog?.countries.orEmpty(), vm.answering) { place, country ->
                    vm.answerUnknown(place, country, onClose)
                }
            }
            SnackbarHost(snackbar, Modifier.align(Alignment.TopCenter))
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun SettingsSheet(vm: AppViewModel, snackbar: SnackbarHostState, onClose: () -> Unit) {
    val colors = LocalAppColors.current
    ModalBottomSheet(onDismissRequest = onClose, containerColor = colors.panel) {
        Box {
            Box(Modifier.verticalScroll(rememberScrollState()).padding(start = 16.dp, end = 16.dp, bottom = 24.dp)) {
                SettingsPanel(vm.server) { AccountSection(vm) }
            }
            SnackbarHost(snackbar, Modifier.align(Alignment.TopCenter))
        }
    }
}
