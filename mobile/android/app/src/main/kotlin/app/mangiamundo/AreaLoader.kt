package app.mangiamundo

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.viewModelScope
import app.mangiamundo.core.AreaProgress
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

private const val NOT_REACHED = "not reached"

/**
 * Loading what the server knows: the countries at startup, then the area's progress with
 * each refresh, the wheel while that loads, and the server's version. Part of [AppViewModel].
 */
internal class AreaLoader(private val vm: AppViewModel) {
    private val api get() = vm.api

    var loading by mutableStateOf(false); private set
    var error by mutableStateOf<String?>(null); private set
    /** How far the server is with the area while it loads; null when there is nothing to show. */
    var areaProgress by mutableStateOf<AreaProgress?>(null); private set
    /** Counts refreshes, so only the latest one shows or clears the progress wheel. */
    var refreshes = 0
        private set
    /** The server's version and commit; null until it answers, "not reached" after a failure. */
    var server by mutableStateOf<String?>(null); private set

    private var refreshJob: Job? = null
    /** A country to open once progress has loaded, from a notification tap. */
    private var pendingCountry: String? = null

    /** Loads the countries, then the area, or lets the welcome screen show without one. */
    fun loadCatalog() {
        vm.viewModelScope.launch {
            try {
                vm.catalog = api.catalog()
            } catch (e: Exception) {
                error = e.message
                vm.ready = true
                return@launch
            }
            // Without a location the welcome screen shows right away; refresh() sets it otherwise.
            if (vm.location != null) refresh() else vm.ready = true
            // Follow the device when there is no location yet or the last one came from GPS.
            if (vm.location?.fromGps != false && DeviceLocation.hasPermission(vm.getApplication())) vm.locate(quiet = true)
        }
    }

    /** Asks for the server's version until it answers once. */
    fun loadServer() {
        if (server != null && server != NOT_REACHED) return
        vm.viewModelScope.launch {
            server = runCatching { api.health() }
                .map { listOfNotNull(it.version ?: "unknown", it.commit).joinToString(" ") }
                .getOrDefault(NOT_REACHED)
        }
    }

    fun refresh() {
        loadServer()
        val here = vm.location ?: return
        refreshJob?.cancel()
        val refresh = ++refreshes
        refreshJob = vm.viewModelScope.launch {
            loading = true
            error = null
            // A wheel from the previous area must not linger while this one starts.
            areaProgress = null
            try {
                val polling = launch { followProgress(here.area.id, refresh) }
                vm.discover = try {
                    api.discover(here.area.id)
                } finally {
                    polling.cancel()
                    // A cancelled older refresh can end after a newer one started: leave its wheel.
                    if (refresh == refreshes) areaProgress = null
                }
                vm.ready = true
                pendingCountry?.let { code ->
                    pendingCountry = null
                    if (vm.discover?.countries?.any { it.code == code } == true) vm.select(code)
                }
                vm.celebrations = runCatching { api.celebrations(here.area.id) }.getOrDefault(emptyList())
                vm.adding.announceConfirmedAdditions()
                vm.reminders.rescheduleReminders()
            } catch (e: kotlinx.coroutines.CancellationException) {
                throw e
            } catch (e: Exception) {
                error = e.message
                vm.discover = null
                vm.ready = true
            }
            loading = false
        }
    }

    /** Polls the area's loading progress, once loading has taken a moment, until cancelled.
     * Only the latest refresh (`refresh`) shows its wheel. */
    private suspend fun followProgress(areaId: String, refresh: Int) {
        delay(300)
        while (true) {
            try {
                // null: not started or just finished; keep what is shown.
                api.areaProgress(areaId)?.takeIf { refresh == refreshes }?.let {
                    areaProgress = it
                    // At startup the loader waits for the data: let the wheel show instead.
                    vm.ready = true
                }
            } catch (e: kotlinx.coroutines.CancellationException) {
                throw e
            } catch (e: Exception) {
                // A missed poll only delays the wheel.
            }
            delay(400)
        }
    }

    /** Opens what a notification points at. */
    fun openFromNotification(country: String?, badges: Boolean) {
        if (badges) {
            vm.select(null)
            vm.tab = 1
        }
        if (country == null) return
        if (vm.discover?.countries?.any { it.code == country } == true) vm.select(country) else pendingCountry = country
    }
}
