package app.mangiamundo

import android.app.Application
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import app.mangiamundo.core.ApiClient
import app.mangiamundo.core.Camera
import app.mangiamundo.core.Catalog
import app.mangiamundo.core.Celebration
import app.mangiamundo.core.Country
import app.mangiamundo.core.Discover
import app.mangiamundo.core.Area
import app.mangiamundo.core.AreaKind
import app.mangiamundo.core.Place
import app.mangiamundo.core.ProjectionKind
import app.mangiamundo.core.ReminderPlan
import app.mangiamundo.core.Status
import app.mangiamundo.core.World
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlin.math.abs

class AppViewModel(application: Application) : AndroidViewModel(application) {
    private val settings = Settings(application)
    private val api = ApiClient(BuildConfig.API_BASE, settings)

    var world by mutableStateOf<World?>(null); private set
    var catalog by mutableStateOf<Catalog?>(null); private set
    var location by mutableStateOf(settings.location); private set
    var discover by mutableStateOf<Discover?>(null); private set
    var loading by mutableStateOf(false); private set
    var error by mutableStateOf<String?>(null); private set
    /** The first data is in (or failed), so the startup loader can go. Read by MainActivity. */
    @Volatile var ready = false; private set

    /** Code of the country whose places are shown. */
    var selected by mutableStateOf<String?>(null); private set
    var places by mutableStateOf<List<Place>>(emptyList()); private set
    var placesLoading by mutableStateOf(false); private set
    var busyPlace by mutableStateOf<String?>(null); private set

    var locating by mutableStateOf(false); private set
    var locateError by mutableStateOf<String?>(null); private set

    var celebrations by mutableStateOf<List<Celebration>>(emptyList()); private set
    var remindersOn by mutableStateOf(settings.remindersOn && Notifications.allowed(application)); private set
    /** 0 = countries, 1 = badges. */
    var tab by mutableStateOf(0)

    var projection by mutableStateOf(settings.projection); private set
    var camera by mutableStateOf(Camera())

    private val messageChannel = Channel<String>(Channel.BUFFERED)
    /** Short announcements (badges, discoveries, errors) for the snackbar. */
    val messages = messageChannel.receiveAsFlow()

    /** The last real device position, used to verify check-ins while browsing another city. */
    private var gpsFix: Triple<Double, Double, Long>? = null
    private var refreshJob: Job? = null
    /** A country to open once progress has loaded, from a notification tap. */
    private var pendingCountry: String? = null
    private var placesJob: Job? = null

    init {
        location?.let { camera = camera.copy(lng = it.lng, lat = it.lat) }
        viewModelScope.launch {
            world = withContext(Dispatchers.IO) {
                World.parse(application.assets.open("world.json").bufferedReader().use { it.readText() })
            }
        }
        viewModelScope.launch {
            try {
                catalog = api.catalog()
            } catch (e: Exception) {
                error = e.message
                ready = true
                return@launch
            }
            if (location == null) settings.legacyLocation?.let { (lat, lng, fromGps) -> moveLegacy(lat, lng, fromGps) }
            // Without a location the welcome screen shows right away; refresh() sets it otherwise.
            if (location != null) refresh() else ready = true
            // Follow the device when there is no location yet or the last one came from GPS.
            if (location?.fromGps != false && DeviceLocation.hasPermission(application)) locate(quiet = true)
        }
    }

    fun country(code: String): Country? = catalog?.countries?.firstOrNull { it.code == code }

    fun changeProjection(kind: ProjectionKind) {
        projection = kind
        settings.projection = kind
    }

    /** Picks a city or postal code from a search. */
    fun chooseArea(area: Area) {
        moveTo(SavedLocation(area, area.lat, area.lng, fromGps = false))
    }

    /** Switches between the city and the postal code found at the GPS position. */
    fun switchArea(area: Area) {
        val current = location ?: return
        if (current.area.id != area.id) moveTo(current.copy(area = area))
    }

    /** Moves a location saved before areas existed to the city at that point. */
    private suspend fun moveLegacy(lat: Double, lng: Double, fromGps: Boolean) {
        val located = runCatching { api.locateArea(lat, lng) }.getOrNull() ?: return
        val area = located.city ?: located.postalCode ?: return
        val target = SavedLocation(area, lat, lng, fromGps, located.options)
        location = target
        settings.location = target
    }

    /** Where the user is, when known recently: distances are measured from it. */
    private fun userPosition(): Pair<Double, Double>? {
        gpsFix?.takeIf { System.currentTimeMillis() - it.third < 10 * 60_000 }?.let { return it.first to it.second }
        return location?.takeIf { it.fromGps }?.let { it.lat to it.lng }
    }

    private fun moveTo(target: SavedLocation) {
        location = target
        settings.location = target
        select(null)
        refresh()
    }

    /** Moves to the device position. With `quiet`, failures leave the current location alone. */
    fun locate(quiet: Boolean = false) {
        if (locating) return
        locating = true
        locateError = null
        viewModelScope.launch {
            val fix = DeviceLocation.current(getApplication())
            if (fix == null) {
                if (!quiet || location == null) {
                    locateError = "Could not determine your location. Search for your city instead."
                }
            } else {
                gpsFix = Triple(fix.latitude, fix.longitude, System.currentTimeMillis())
                val previous = location
                val moved = previous == null || !previous.fromGps ||
                    abs(previous.lat - fix.latitude) > 0.01 || abs(previous.lng - fix.longitude) > 0.01
                // Background refreshes skip small moves; an explicit request always looks the area
                // up, since a short walk can cross a postal-code boundary.
                if (moved || !quiet) {
                    val located = runCatching { api.locateArea(fix.latitude, fix.longitude) }.getOrNull()
                    // The city by default; someone who narrowed to their postal code keeps that. Read
                    // the selection now: it may have changed while the lookup ran.
                    val current = location
                    val keepPostalCode = current?.fromGps == true && current.area.kind == AreaKind.POSTAL_CODE
                    val area = located?.let { (if (keepPostalCode) it.postalCode else null) ?: it.city ?: it.postalCode }
                    if (located == null || area == null) {
                        if (!quiet || location == null) {
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

    suspend fun search(query: String): List<Area> = api.searchAreas(query)

    suspend fun suggest(query: String): List<Area> =
        api.suggestAreas(query, gpsFix?.let { it.first to it.second } ?: location?.let { it.lat to it.lng })

    fun refresh() {
        val here = location ?: return
        refreshJob?.cancel()
        refreshJob = viewModelScope.launch {
            loading = true
            error = null
            try {
                discover = api.discover(here.area.id)
                ready = true
                pendingCountry?.let { code ->
                    pendingCountry = null
                    if (discover?.countries?.any { it.code == code } == true) select(code)
                }
                celebrations = runCatching { api.celebrations(here.area.id) }.getOrDefault(emptyList())
                rescheduleReminders()
            } catch (e: kotlinx.coroutines.CancellationException) {
                throw e
            } catch (e: Exception) {
                error = e.message
                discover = null
                ready = true
            }
            loading = false
        }
    }

    /** Opens what a notification points at. */
    fun openFromNotification(country: String?, badges: Boolean) {
        if (badges) {
            select(null)
            tab = 1
        }
        if (country == null) return
        if (discover?.countries?.any { it.code == country } == true) select(country) else pendingCountry = country
    }

    /** Call after the notification permission was granted (or with false when refused). */
    fun setReminders(on: Boolean) {
        val app = getApplication<Application>()
        remindersOn = on && Notifications.allowed(app)
        settings.remindersOn = remindersOn
        if (remindersOn) {
            rescheduleReminders()
            messageChannel.trySend("Reminders on. We will let you know when a country near you has something to celebrate.")
        } else {
            Notifications.cancelAll(app)
            if (on) messageChannel.trySend("Notifications are turned off for Mangiamundo in your settings.")
        }
    }

    private fun rescheduleReminders() {
        if (!remindersOn) return
        val plan = ReminderPlan.plan(celebrations, discover?.events.orEmpty(), java.time.ZonedDateTime.now())
        Notifications.schedule(getApplication(), plan)
    }

    fun select(code: String?) {
        selected = code
        places = emptyList()
        placesJob?.cancel()
        val here = location
        if (code == null || here == null) {
            placesLoading = false
            return
        }
        placesJob = viewModelScope.launch {
            placesLoading = true
            try {
                places = api.places(here.area.id, code, userPosition())
            } catch (e: kotlinx.coroutines.CancellationException) {
                throw e
            } catch (e: Exception) {
                messageChannel.trySend("Could not load places: ${e.message}")
            }
            placesLoading = false
        }
    }

    /** A tap on the map: open the country if it is available here, otherwise say so. */
    fun tapCountry(code: String, name: String) {
        if (discover?.countries?.any { it.code == code } == true) {
            select(code)
        } else if (discover != null) {
            messageChannel.trySend("No places from $name in ${location?.area?.name ?: "this area"}.")
        }
    }

    fun toggleVisit(place: Place) {
        val before = discover ?: return
        val here = location ?: return
        if (busyPlace != null) return
        busyPlace = place.id
        viewModelScope.launch {
            try {
                val updated = if (!place.visited) {
                    val fix = gpsFix?.takeIf { System.currentTimeMillis() - it.third < 10 * 60_000 }
                    val result = api.checkIn(place.id, fix?.let { it.first to it.second })
                    place.copy(visitedAt = System.currentTimeMillis() / 1000, verified = result.verified)
                } else {
                    api.undoCheckIn(place.id)
                    place.copy(visitedAt = null, verified = false)
                }
                places = places.map { if (it.id == place.id) updated else it }
                val after = api.discover(here.area.id)
                discover = after
                if (updated.visited) celebrate(before, after, updated)
            } catch (e: Exception) {
                messageChannel.trySend("That did not work: ${e.message}")
            }
            busyPlace = null
        }
    }

    /** Announces everything a check-in unlocked by comparing progress before and after. */
    private fun celebrate(before: Discover, after: Discover, place: Place) {
        for (code in place.countries) {
            val was = before.countries.firstOrNull { it.code == code } ?: continue
            val now = after.countries.firstOrNull { it.code == code } ?: continue
            val info = country(code) ?: continue
            if (now.status == Status.COMPLETED && was.status != Status.COMPLETED) {
                messageChannel.trySend("${info.flag} ${info.name} complete")
            } else if (was.visited == 0) {
                messageChannel.trySend("${info.flag} ${info.name} tasted. ${now.places - now.visited} more to complete it.")
            }
        }
        for (badge in after.badges) {
            if (badge.earned && before.badges.none { it.id == badge.id && it.earned }) {
                messageChannel.trySend("Badge earned: ${badge.title}")
            }
        }
        for (event in after.events) {
            if (event.earned && before.events.none { it.id == event.id && it.earned }) {
                messageChannel.trySend("Badge earned: ${event.title}")
            }
        }
        if (after.profile.level > before.profile.level) {
            messageChannel.trySend("Level ${after.profile.level}")
        }
    }
}
