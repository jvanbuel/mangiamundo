package app.mangiamundo

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.viewModelScope
import app.mangiamundo.core.Place
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch

/** The selected country's places, and the place whose details are open. Part of [AppViewModel]. */
internal class PlacesModel(private val vm: AppViewModel) {
    private val api get() = vm.api

    /** Code of the country whose places are shown. */
    var selected by mutableStateOf<String?>(null); private set
    var placesLoading by mutableStateOf(false); private set
    /** Whether the details hold all visits yet. Until then they show the card's latest one. */
    var detailVisits by mutableStateOf(DetailVisits.LOADING); private set
    private var detailJob: Job? = null
    private var placesJob: Job? = null

    fun select(code: String?) {
        selected = code
        vm.places = emptyList()
        placesJob?.cancel()
        val here = vm.location
        if (code == null || here == null) {
            placesLoading = false
            return
        }
        placesJob = vm.viewModelScope.launch {
            placesLoading = true
            try {
                vm.places = api.places(here.area.id, code, vm.userPosition())
            } catch (e: kotlinx.coroutines.CancellationException) {
                throw e
            } catch (e: Exception) {
                vm.messageChannel.trySend("Could not load places: ${e.message}")
            }
            placesLoading = false
        }
    }

    /** Opens a place's details, with all of the user's visits. */
    fun openPlace(place: Place) {
        vm.detail = place
        detailVisits = DetailVisits.LOADING
        detailJob?.cancel()
        // Measured from the user when known, otherwise from the map pin.
        val from = vm.userPosition() ?: vm.location?.let { it.lat to it.lng }
        detailJob = vm.viewModelScope.launch {
            try {
                vm.detail = api.place(place.id, from)
                detailVisits = DetailVisits.LOADED
            } catch (e: kotlinx.coroutines.CancellationException) {
                throw e
            } catch (e: Exception) {
                detailVisits = DetailVisits.FAILED
            }
        }
    }

    fun closePlace() {
        detailJob?.cancel()
        vm.detail = null
    }

    /** A tap on the map: open the country if it is available here, otherwise say so. */
    fun tapCountry(code: String, name: String) {
        if (vm.discover?.countries?.any { it.code == code } == true) {
            select(code)
        } else if (vm.discover != null) {
            vm.messageChannel.trySend("No places from $name in ${vm.location?.area?.name ?: "this area"}.")
        }
    }
}
