package app.mangiamundo

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.viewModelScope
import app.mangiamundo.core.Place
import app.mangiamundo.core.Refreshed
import kotlinx.coroutines.launch

/** Places read again from OpenStreetMap, after an edit there. Part of [AppViewModel]. */
internal class OsmRefreshModel(private val vm: AppViewModel) {
    private val api get() = vm.api
    private val messageChannel get() = vm.messageChannel

    /** The place being read again from OpenStreetMap. */
    var refreshingPlace by mutableStateOf<String?>(null); private set
    /** True while OpenStreetMap is asked for places just mapped nearby. */
    var lookingNearby by mutableStateOf(false); private set
    /** The place the user went to edit in an OpenStreetMap app, read again when they come back. */
    private var editing: String? = null

    fun editingOnOsm(placeId: String) {
        editing = placeId
    }

    /** A refresh waiting for the one running to finish. */
    private var queuedRefresh: String? = null

    /** Back in the app: shows an edit just made to a place on OpenStreetMap. */
    fun onResume() {
        val id = editing ?: return
        editing = null
        // A refresh still running may predate the edit: this one goes after it.
        if (refreshingPlace != null) queuedRefresh = id else refreshPlace(id, quiet = true)
    }

    /** Reads a place again from OpenStreetMap and says what changed. With `quiet`, a failure
     * isn't shown, as when it runs by itself after the user comes back from editing. */
    fun refreshPlace(placeId: String, quiet: Boolean = false) {
        if (refreshingPlace != null) return
        refreshingPlace = placeId
        val name = vm.detail?.takeIf { it.id == placeId }?.name ?: vm.places.firstOrNull { it.id == placeId }?.name ?: "This place"
        vm.viewModelScope.launch {
            try {
                val result = api.refreshPlace(placeId, vm.userPosition())
                val place = result.place
                if (result.status == Refreshed.GONE || place == null) {
                    vm.places = vm.places.filter { it.id != placeId }
                    if (vm.detail?.id == placeId) vm.closePlace()
                    messageChannel.trySend("$name is gone from OpenStreetMap, so it's hidden now. Your visits to it are kept.")
                } else {
                    // Also when unchanged: someone else's refresh may have updated it since it loaded.
                    applyRefreshed(place)
                    messageChannel.trySend(
                        if (result.status == Refreshed.UPDATED) "${place.name} updated from OpenStreetMap."
                        else "No change on OpenStreetMap yet. A new edit can take a minute or two to show up there.",
                    )
                }
                // Its countries may have changed, also with an unchanged reply when someone else's
                // refresh changed it first. A reply for an area the user has left is dropped.
                val refresh = vm.refreshes
                vm.location?.let { here ->
                    runCatching { api.discover(here.area.id) }.onSuccess { if (refresh == vm.refreshes) vm.discover = it }
                }
            } catch (e: kotlinx.coroutines.CancellationException) {
                throw e
            } catch (e: Exception) {
                if (!quiet) messageChannel.trySend("Could not refresh this place: ${e.message}")
            } finally {
                refreshingPlace = null
                queuedRefresh?.let { next ->
                    queuedRefresh = null
                    refreshPlace(next, quiet = true)
                }
            }
        }
    }

    /** Shows a place read again from OpenStreetMap. Visits stay as shown: a check-in or undo
     * made while the refresh ran is newer than the reply. A place no longer serving the
     * selected country's food leaves that list. */
    private fun applyRefreshed(place: Place) {
        vm.detail?.let { if (it.id == place.id) vm.detail = place.copy(visits = it.visits) }
        vm.places = vm.places
            .map { if (it.id == place.id) place.copy(visits = it.visits) else it }
            .filter { it.id != place.id || vm.selected == null || vm.selected in it.countries }
    }

    /** Asks OpenStreetMap for restaurants just mapped around the user, for someone who added
     * one in an OpenStreetMap app instead of the add form. */
    fun lookAgainNearby() {
        if (lookingNearby) return
        lookingNearby = true
        vm.viewModelScope.launch {
            try {
                val fix = DeviceLocation.current(vm.getApplication())
                if (fix == null) {
                    messageChannel.trySend("Could not determine your location. Try again outside, or with location turned on.")
                } else {
                    vm.gpsFix = Triple(fix.latitude, fix.longitude, System.currentTimeMillis())
                    val (added, matched) = api.refreshNearby(fix.latitude, fix.longitude)
                    if (added.isEmpty() && matched == 0) {
                        messageChannel.trySend("Nothing new within 150 m yet. A place just added on OpenStreetMap can take a minute or two to show up.")
                    } else {
                        if (added.isNotEmpty()) messageChannel.trySend("Found ${added.joinToString { it.name }}: on the map for everyone now.")
                        // New countries, and any of the user's additions they confirmed.
                        vm.refresh()
                        // The open country's list, with the places just found.
                        vm.selected?.let { vm.select(it) }
                    }
                }
            } catch (e: Exception) {
                messageChannel.trySend("Could not look again: ${e.message}")
            }
            lookingNearby = false
        }
    }
}
