package app.mangiamundo

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.viewModelScope
import app.mangiamundo.core.Area
import app.mangiamundo.core.AreaKind
import app.mangiamundo.core.withRecent
import kotlinx.coroutines.launch
import kotlin.math.abs

/** Where the user is looking: the area, searched for or found at the device. Part of [AppViewModel]. */
internal class LocationModel(private val vm: AppViewModel) {
    private val api get() = vm.api
    private val settings get() = vm.settings

    /** Areas chosen recently, newest first, for the location picker. Includes the current one. */
    var recentAreas by mutableStateOf(settings.recentAreas); private set
    var locating by mutableStateOf(false); private set
    var locateError by mutableStateOf<String?>(null); private set

    /** Picks a city or postal code from a search. */
    fun chooseArea(area: Area) {
        moveTo(SavedLocation(area, area.lat, area.lng, fromGps = false))
    }

    /** Switches between the city and the postal code found at the GPS position. */
    fun switchArea(area: Area) {
        val current = vm.location ?: return
        if (current.area.id != area.id) moveTo(current.copy(area = area))
    }

    /** Takes an area off the recent list. */
    fun forgetRecent(id: String) {
        recentAreas = recentAreas.filter { it.id != id }
        settings.recentAreas = recentAreas
    }

    private fun moveTo(target: SavedLocation) {
        // The area being left too, so one saved before recents existed shows up.
        recentAreas = withRecent(vm.location?.let { withRecent(recentAreas, it.area) } ?: recentAreas, target.area)
        settings.recentAreas = recentAreas
        vm.location = target
        vm.spins.forgetSpun()
        // The old area's countries must not show while the new area loads.
        vm.discover = null
        vm.celebrations = emptyList()
        settings.location = target
        vm.closePlace()
        vm.select(null)
        vm.refresh()
    }

    /** Moves to the device position. With `quiet`, failures leave the current location alone. */
    fun locate(quiet: Boolean = false) {
        if (locating) return
        locating = true
        locateError = null
        vm.viewModelScope.launch {
            val fix = DeviceLocation.current(vm.getApplication())
            if (fix == null) {
                if (!quiet || vm.location == null) {
                    locateError = "Could not determine your location. Search for your city instead."
                }
            } else {
                vm.gpsFix = Triple(fix.latitude, fix.longitude, System.currentTimeMillis())
                val previous = vm.location
                val moved = previous == null || !previous.fromGps ||
                    abs(previous.lat - fix.latitude) > 0.01 || abs(previous.lng - fix.longitude) > 0.01
                // Background refreshes skip small moves; an explicit request always looks the area
                // up, since a short walk can cross a postal-code boundary.
                if (moved || !quiet) {
                    val located = runCatching { api.locateArea(fix.latitude, fix.longitude) }.getOrNull()
                    // The city by default; someone who narrowed to their postal code keeps that. Read
                    // the selection now: it may have changed while the lookup ran.
                    val current = vm.location
                    val keepPostalCode = current?.fromGps == true && current.area.kind == AreaKind.POSTAL_CODE
                    val area = located?.let { (if (keepPostalCode) it.postalCode else null) ?: it.city ?: it.postalCode }
                    if (located == null || area == null) {
                        if (!quiet || vm.location == null) {
                            locateError = "No city or postal code found here. Search for your city instead."
                        }
                    } else {
                        moveTo(SavedLocation(area, fix.latitude, fix.longitude, fromGps = true, options = located.options))
                    }
                }
            }
            locating = false
        }
    }

    fun locationDenied() {
        locateError = "Location access was denied. Search for your city instead."
    }
}
