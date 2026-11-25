package app.mangiamundo

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.viewModelScope
import app.mangiamundo.core.ApiException
import app.mangiamundo.core.Discover
import app.mangiamundo.core.Place
import app.mangiamundo.core.Visit
import app.mangiamundo.core.VisitRemoved
import app.mangiamundo.core.VisitResult
import kotlinx.coroutines.launch

/** Check-ins, their undoing, and reports of closed places. Part of [AppViewModel]. */
internal class VisitsModel(private val vm: AppViewModel) {
    private val api get() = vm.api
    private val messageChannel get() = vm.messageChannel

    /** The place a check-in, undo or report is running for. One at a time. */
    var busyPlace by mutableStateOf<String?>(null); private set

    fun toggleVisit(place: Place) {
        if (vm.discover == null) return
        val here = vm.location ?: return
        if (busyPlace != null) return
        busyPlace = place.id
        val generation = vm.accountGeneration
        // Nothing of the answer is shown once the device is in another account.
        val sameAccount = { vm.accountGeneration == generation }
        // Progress is only shown when it is also still about this area.
        val stillHere = { vm.location?.area?.id == here.area.id && sameAccount() }
        vm.viewModelScope.launch {
            try {
                // Visited today: undo that visit. Otherwise check in, also when there are older visits.
                val undoing = place.visitedToday()
                var checkedIn: VisitResult? = null
                var progress: Discover? = null
                val update: (List<Visit>) -> List<Visit> = if (!undoing) {
                    val fix = vm.gpsFix?.takeIf { System.currentTimeMillis() - it.third < 10 * 60_000 }
                    val result = api.checkIn(place.id, here.area.id, fix?.let { it.first to it.second })
                    checkedIn = result
                    progress = result.progress
                    val visit = result.visit
                    // Checking in twice on one day returns that day's visit.
                    { old -> listOf(visit) + old.filter { it.id != visit.id } }
                } else {
                    val result = try {
                        api.removeVisit(place.visits.first().id, here.area.id)
                    } catch (e: ApiException) {
                        if (e.code != "not_found") throw e
                        // Gone already, from another device or by a retry whose answer was lost: ask what is left.
                        val after = api.discover(here.area.id)
                        VisitRemoved(api.place(place.id, null).visits, after.profile, after)
                    }
                    progress = result.progress
                    // The server's list of the visits left: the card only knew the latest.
                    { _ -> result.visits }
                }
                if (!sameAccount()) return@launch
                setVisits(place.id, update)
                // Details still loading may have read the visits from before this change.
                vm.detail?.takeIf { it.id == place.id && vm.detailVisits != DetailVisits.LOADED }?.let { vm.openPlace(it) }
                progress?.let { if (stillHere()) vm.discover = it }
                for (unlocked in checkedIn?.unlocked.orEmpty()) messageChannel.trySend(unlocked.announcement(vm.catalog))
            } catch (e: Exception) {
                messageChannel.trySend("That did not work: ${e.message}")
            } finally {
                busyPlace = null
            }
        }
    }

    /** Reports a place as closed for good: it disappears from this user's lists right away. */
    fun reportClosed(place: Place) {
        if (busyPlace != null) return
        busyPlace = place.id
        vm.viewModelScope.launch {
            try {
                val here = vm.location
                val generation = vm.accountGeneration
                val result = api.reportClosed(place.id, here?.area?.id)
                if (vm.accountGeneration != generation) return@launch
                vm.places = vm.places.filter { it.id != place.id }
                if (vm.detail?.id == place.id) vm.closePlace()
                messageChannel.trySend("Thanks for letting us know. ${place.name} is hidden for you now.")
                // The place no longer counts towards its countries.
                val progress = result.progress
                if (progress != null && here != null && vm.location?.area?.id == here.area.id) vm.discover = progress
            } catch (e: Exception) {
                messageChannel.trySend("Could not report this place: ${e.message}")
            } finally {
                busyPlace = null
            }
        }
    }

    /** Updates a place's visits on its card (which shows only the latest) and in its details. */
    private fun setVisits(id: String, update: (List<Visit>) -> List<Visit>) {
        vm.places = vm.places.map { if (it.id == id) it.copy(visits = update(it.visits).take(1)) else it }
        vm.detail = vm.detail?.let { if (it.id == id) it.copy(visits = update(it.visits)) else it }
    }
}
