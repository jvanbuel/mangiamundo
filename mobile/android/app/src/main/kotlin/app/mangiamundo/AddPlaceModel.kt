package app.mangiamundo

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.viewModelScope
import app.mangiamundo.core.UnknownPlace
import app.mangiamundo.core.pendingText
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * Restaurants the user adds: new ones, the food of ones with unknown food, and news of
 * those OpenStreetMap now has. Part of [AppViewModel].
 */
internal class AddPlaceModel(private val vm: AppViewModel) {
    private val api get() = vm.api
    private val messageChannel get() = vm.messageChannel

    /** True while a new place is being added. */
    var addingPlace by mutableStateOf(false); private set

    /** Where the add form's pin starts: the device's position, or the area's centre without one. */
    suspend fun startingPoint(): Pair<Double, Double> {
        DeviceLocation.current(vm.getApplication())?.let { fix ->
            vm.gpsFix = Triple(fix.latitude, fix.longitude, System.currentTimeMillis())
            return fix.latitude to fix.longitude
        }
        return vm.userPosition() ?: ((vm.location?.lat ?: 0.0) to (vm.location?.lng ?: 0.0))
    }

    /**
     * Adds a restaurant OpenStreetMap lacks, where the user put the pin. Opens it once added.
     * `note = false` when the user adds it to OSM themselves, so no note asks mappers to.
     */
    fun addPlace(name: String, country: String, lat: Double, lng: Double, address: String?, website: String?, note: Boolean, onAdded: () -> Unit) {
        if (addingPlace) return
        addingPlace = true
        vm.viewModelScope.launch {
            try {
                val (place, reward) = api.addPlace(name, country, lat, lng, address, website, note)
                val earns = reward.pendingText(country.uppercase(), vm.catalog)
                messageChannel.trySend(
                    if (note) "${place.name} added. You see it right away. $earns"
                    else "${place.name} added for you. Now add it to the map in the other app, so everyone gets it. $earns",
                )
                onAdded()
                vm.openPlace(place)
                countIn()
            } catch (e: Exception) {
                messageChannel.trySend("Could not add the restaurant: ${e.message}")
            }
            addingPlace = false
        }
    }

    /** Refreshes the progress after a pending place was added: it counts towards its country,
     * for this user, if it lies in the area. Applied only if no refresh (such as a change of
     * area) started meanwhile. */
    private suspend fun countIn() {
        val refresh = vm.refreshes
        vm.location?.let { here ->
            runCatching { api.discover(here.area.id) }.onSuccess { if (refresh == vm.refreshes) vm.discover = it }
        }
    }

    /** Restaurants in the area with unknown food, nearest to the user first. */
    var unknownPlaces by mutableStateOf<List<UnknownPlace>>(emptyList()); private set
    /** The unknown place whose answer is being saved. One at a time. */
    var answering by mutableStateOf<String?>(null); private set

    /** Loads [unknownPlaces] for the current area. A failure leaves the list empty. */
    fun loadUnknownPlaces() {
        val here = vm.location ?: return
        vm.viewModelScope.launch {
            val found = runCatching { api.unknownPlaces(here.area.id, vm.userPosition()) }.getOrDefault(emptyList())
            if (vm.location?.area?.id == here.area.id) unknownPlaces = found
        }
    }

    /** "It serves food from …" (or, with a null `country`, "nothing specific") for a restaurant
     * with unknown food. A suggestion opens the place it adds, after `onAdded`. */
    fun answerUnknown(unknown: UnknownPlace, country: String?, onAdded: () -> Unit) {
        if (answering != null) return
        answering = unknown.id
        vm.viewModelScope.launch {
            try {
                if (country == null) {
                    api.noCuisine(unknown.id)
                } else {
                    val (place, reward) = api.suggestCuisine(unknown.id, country)
                    messageChannel.trySend("${place.name} added. You see it right away. ${reward.pendingText(country.uppercase(), vm.catalog)}")
                    onAdded()
                    vm.openPlace(place)
                }
                unknownPlaces = unknownPlaces.filter { it.id != unknown.id }
                if (country != null) countIn()
            } catch (e: Exception) {
                messageChannel.trySend("Could not save that: ${e.message}")
            }
            answering = null
        }
    }

    /** "Lalibela is on the map. +130 XP…" for each addition OpenStreetMap now has that the
     * user wasn't told about. Only those queued are marked seen; the rest wait for the next refresh. */
    suspend fun announceConfirmedAdditions() {
        // A few at a time: any more wait for the next refresh.
        val news = runCatching { api.additions().filter { it.news }.take(10) }.getOrNull() ?: return
        val told = mutableListOf<String>()
        try {
            // `send` waits for room, where `trySend` would drop messages past the buffer.
            for (addition in news) {
                messageChannel.send(addition.announcement(vm.catalog))
                told += addition.id
            }
        } finally {
            if (told.isNotEmpty()) withContext(NonCancellable) { runCatching { api.markAdditionsSeen(told) } }
        }
    }
}
