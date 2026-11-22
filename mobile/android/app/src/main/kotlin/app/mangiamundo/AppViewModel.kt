package app.mangiamundo

import android.app.Application
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import app.mangiamundo.core.Account
import app.mangiamundo.core.ApiClient
import app.mangiamundo.core.ApiException
import app.mangiamundo.core.LinkCode
import app.mangiamundo.core.ShareLink
import app.mangiamundo.ui.shareLink
import app.mangiamundo.core.Camera
import app.mangiamundo.core.Catalog
import app.mangiamundo.core.Celebration
import app.mangiamundo.core.Country
import app.mangiamundo.core.Discover
import app.mangiamundo.core.Area
import app.mangiamundo.core.AreaKind
import app.mangiamundo.core.AreaProgress
import app.mangiamundo.core.Place
import app.mangiamundo.core.Refreshed
import app.mangiamundo.core.pendingText
import app.mangiamundo.core.ProjectionKind
import app.mangiamundo.core.ReminderPlan
import app.mangiamundo.core.Status
import app.mangiamundo.ui.label
import app.mangiamundo.core.Visit
import app.mangiamundo.core.World
import app.mangiamundo.core.withRecent
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlin.math.abs

private const val NOT_REACHED = "not reached"
/** How long the globe turns at least before a spin lands, and how long one turn takes. */
private const val SPIN_MS = 1600L

class AppViewModel(application: Application) : AndroidViewModel(application) {
    private val settings = Settings(application)
    private val api = ApiClient(BuildConfig.API_BASE, settings, android.os.Build.MODEL)

    var world by mutableStateOf<World?>(null); private set
    var catalog by mutableStateOf<Catalog?>(null); private set
    var location by mutableStateOf(settings.location); private set
    /** Areas chosen recently, newest first, for the location picker. Includes the current one. */
    var recentAreas by mutableStateOf(settings.recentAreas); private set
    var discover by mutableStateOf<Discover?>(null); private set
    var loading by mutableStateOf(false); private set
    var error by mutableStateOf<String?>(null); private set
    /** How far the server is with the area while it loads; null when there is nothing to show. */
    var areaProgress by mutableStateOf<AreaProgress?>(null); private set
    /** Counts refreshes, so only the latest one shows or clears the progress wheel. */
    private var refreshes = 0
    /** The first data is in (or failed), so the startup loader can go. Read by MainActivity. */
    @Volatile var ready = false; private set

    /** Code of the country whose places are shown. */
    var selected by mutableStateOf<String?>(null); private set
    var places by mutableStateOf<List<Place>>(emptyList()); private set
    var placesLoading by mutableStateOf(false); private set
    var busyPlace by mutableStateOf<String?>(null); private set
    /** The place whose details are open: its card's data until the full place has loaded. */
    var detail by mutableStateOf<Place?>(null); private set
    /** Whether the details hold all visits yet. Until then they show the card's latest one. */
    var detailVisits by mutableStateOf(DetailVisits.LOADING); private set
    private var detailJob: Job? = null

    /** A spin is running: the globe turns and the Spin buttons wait. */
    var spinning by mutableStateOf(false); private set
    /** The place the last spin landed on, whose details offer to spin again. */
    var spun by mutableStateOf<String?>(null); private set
    /** Places suggested since the area was picked, so spinning again finds another. */
    private var spunIds = mutableListOf<String>()

    var locating by mutableStateOf(false); private set
    var locateError by mutableStateOf<String?>(null); private set

    var celebrations by mutableStateOf<List<Celebration>>(emptyList()); private set
    var remindersOn by mutableStateOf(settings.remindersOn && Notifications.allowed(application)); private set
    /** 0 = countries, 1 = badges. */
    var tab by mutableStateOf(0)
    /** The server's version and commit; null until it answers, "not reached" after a failure. */
    var server by mutableStateOf<String?>(null); private set

    /** The account's recovery code status and devices, loaded when settings open. */
    var account by mutableStateOf<Account?>(null); private set
    var accountError by mutableStateOf<String?>(null); private set
    var accountBusy by mutableStateOf(false); private set
    /** Shown once, right after it is made. */
    var recoveryCode by mutableStateOf<String?>(null); private set
    var linkCode by mutableStateOf<LinkCode?>(null); private set

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
        loadServer()
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

    /** Takes an area off the recent list. */
    fun forgetRecent(id: String) {
        recentAreas = recentAreas.filter { it.id != id }
        settings.recentAreas = recentAreas
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
        // The area being left too, so one saved before recents existed shows up.
        recentAreas = withRecent(location?.let { withRecent(recentAreas, it.area) } ?: recentAreas, target.area)
        settings.recentAreas = recentAreas
        location = target
        spunIds.clear()
        // The old area's countries must not show while the new area loads.
        discover = null
        celebrations = emptyList()
        settings.location = target
        closePlace()
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

    /** Asks for the server's version until it answers once. */
    private fun loadServer() {
        if (server != null && server != NOT_REACHED) return
        viewModelScope.launch {
            server = runCatching { api.health() }
                .map { listOfNotNull(it.version ?: "unknown", it.commit).joinToString(" ") }
                .getOrDefault(NOT_REACHED)
        }
    }

    fun refresh() {
        loadServer()
        val here = location ?: return
        refreshJob?.cancel()
        val refresh = ++refreshes
        refreshJob = viewModelScope.launch {
            loading = true
            error = null
            // A wheel from the previous area must not linger while this one starts.
            areaProgress = null
            try {
                val polling = launch { followProgress(here.area.id, refresh) }
                discover = try {
                    api.discover(here.area.id)
                } finally {
                    polling.cancel()
                    // A cancelled older refresh can end after a newer one started: leave its wheel.
                    if (refresh == refreshes) areaProgress = null
                }
                ready = true
                pendingCountry?.let { code ->
                    pendingCountry = null
                    if (discover?.countries?.any { it.code == code } == true) select(code)
                }
                celebrations = runCatching { api.celebrations(here.area.id) }.getOrDefault(emptyList())
                announceConfirmedAdditions()
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
                    ready = true
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

    /** Opens a place's details, with all of the user's visits. */
    fun openPlace(place: Place) {
        detail = place
        detailVisits = DetailVisits.LOADING
        detailJob?.cancel()
        // Measured from the user when known, otherwise from the map pin.
        val from = userPosition() ?: location?.let { it.lat to it.lng }
        detailJob = viewModelScope.launch {
            try {
                detail = api.place(place.id, from)
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
        detail = null
    }

    /** Spins the globe: it lands on a country and opens an unvisited place there, open now if any is. */
    fun spin() {
        val here = location ?: return
        if (spinning) return
        spinning = true
        closePlace()
        viewModelScope.launch {
            val still = animationsOff()
            val turning = if (still) null else launch { turn() }
            val started = System.currentTimeMillis()
            val exclude = spunIds + settings.notToday.keys
            try {
                val found = try {
                    api.suggestion(here.area.id, userPosition(), exclude, open = true)
                } catch (e: ApiException) {
                    if (e.code != "nothing_open") throw e
                    messageChannel.trySend("Nothing open right now. Here is one for another time.")
                    api.suggestion(here.area.id, userPosition(), exclude, open = false)
                }
                if (!still) delay(SPIN_MS - (System.currentTimeMillis() - started))
                turning?.cancelAndJoin()
                if (location != here) return@launch
                spunIds += found.place.id
                spun = found.place.id
                select(found.country)
                world?.countries?.firstOrNull { it.code == found.country }?.let { glide(it.landing(), still) }
                openPlace(found.place)
            } catch (e: kotlinx.coroutines.CancellationException) {
                throw e
            } catch (e: Exception) {
                if (e is ApiException && e.code == "nothing_left") {
                    if (spunIds.isNotEmpty()) {
                        spunIds.clear()
                        messageChannel.trySend("That was every place. Spin again to start over.")
                    } else {
                        messageChannel.trySend("You have been to every place in ${here.area.name}.")
                    }
                } else {
                    messageChannel.trySend("Could not spin: ${e.message}")
                }
            } finally {
                turning?.cancel()
                spinning = false
            }
        }
    }

    /** "Not today": keeps the place out of spins for 20 hours, and spins again. */
    fun notToday(placeId: String) {
        settings.notToday = settings.notToday + (placeId to System.currentTimeMillis() + 20 * 3_600_000L)
        spin()
    }

    /** Turns the globe one turn per SPIN_MS, zoomed out to all of it, until cancelled. */
    private suspend fun turn() {
        val from = camera
        val started = System.nanoTime()
        while (true) {
            val ms = (System.nanoTime() - started) / 1e6
            val out = minOf(1.0, ms / 500)
            camera = from.copy(lng = from.lng + 360 * ms / SPIN_MS, zoom = from.zoom + (1.0 - from.zoom) * out).normalized()
            delay(16)
        }
    }

    /** Moves the camera to `to`, slowing down at the end; the short way around the globe. */
    private suspend fun glide(to: Camera, still: Boolean) {
        val from = camera
        if (still) {
            camera = to
            return
        }
        val delta = ((to.lng - from.lng + 540) % 360) - 180
        val started = System.nanoTime()
        while (true) {
            val t = minOf(1.0, (System.nanoTime() - started) / 1e6 / 900)
            val e = 1 - (1 - t) * (1 - t) * (1 - t)
            camera = Camera(from.lng + delta * e, from.lat + (to.lat - from.lat) * e, from.zoom + (to.zoom - from.zoom) * e).normalized()
            if (t >= 1) return
            delay(16)
        }
    }

    /** Animations turned off in the system settings: spins land straight away. */
    private fun animationsOff() = android.provider.Settings.Global.getFloat(
        getApplication<Application>().contentResolver,
        android.provider.Settings.Global.ANIMATOR_DURATION_SCALE,
        1f,
    ) == 0f

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
                // Visited today: undo that visit. Otherwise check in, also when there are older visits.
                val undoing = place.visitedToday()
                val update: (List<Visit>) -> List<Visit> = if (!undoing) {
                    val fix = gpsFix?.takeIf { System.currentTimeMillis() - it.third < 10 * 60_000 }
                    val visit = api.checkIn(place.id, fix?.let { it.first to it.second }).visit
                    // Checking in twice on one day returns that day's visit.
                    { old -> listOf(visit) + old.filter { it.id != visit.id } }
                } else {
                    val removed = place.visits.first().id
                    api.removeVisit(removed);
                    { old -> old.filter { it.id != removed } }
                }
                setVisits(place.id, update)
                // Details still loading may have read the visits from before this change.
                detail?.takeIf { it.id == place.id && detailVisits != DetailVisits.LOADED }?.let { openPlace(it) }
                if (undoing) {
                    // The server has the remaining visits; details shown here may be out of date.
                    val rest = runCatching { api.place(place.id, null).visits }.getOrNull()
                    val loaded = detail?.takeIf { it.id == place.id && detailVisits == DetailVisits.LOADED }
                    if (rest != null) {
                        setVisits(place.id) { rest }
                    } else if (loaded != null) {
                        // The card shows only the latest visit: fall back on the older ones in the details.
                        places = places.map { if (it.id == place.id) it.copy(visits = loaded.visits.take(1)) else it }
                    }
                }
                val after = api.discover(here.area.id)
                discover = after
                if (!undoing) celebrate(before, after, place)
            } catch (e: Exception) {
                messageChannel.trySend("That did not work: ${e.message}")
            }
            busyPlace = null
        }
    }

    /** Reports a place as closed for good: it disappears from this user's lists right away. */
    fun reportClosed(place: Place) {
        if (busyPlace != null) return
        busyPlace = place.id
        viewModelScope.launch {
            try {
                api.reportClosed(place.id)
                places = places.filter { it.id != place.id }
                if (detail?.id == place.id) closePlace()
                messageChannel.trySend("Thanks for letting us know. ${place.name} is hidden for you now.")
                // The place no longer counts towards its countries.
                location?.let { discover = api.discover(it.area.id) }
            } catch (e: Exception) {
                messageChannel.trySend("Could not report this place: ${e.message}")
            }
            busyPlace = null
        }
    }

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
        val name = detail?.takeIf { it.id == placeId }?.name ?: places.firstOrNull { it.id == placeId }?.name ?: "This place"
        viewModelScope.launch {
            try {
                val result = api.refreshPlace(placeId, userPosition())
                val place = result.place
                if (result.status == Refreshed.GONE || place == null) {
                    places = places.filter { it.id != placeId }
                    if (detail?.id == placeId) closePlace()
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
                val refresh = refreshes
                location?.let { here ->
                    runCatching { api.discover(here.area.id) }.onSuccess { if (refresh == refreshes) discover = it }
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
        detail?.let { if (it.id == place.id) detail = place.copy(visits = it.visits) }
        places = places
            .map { if (it.id == place.id) place.copy(visits = it.visits) else it }
            .filter { it.id != place.id || selected == null || selected in it.countries }
    }

    /** Asks OpenStreetMap for restaurants just mapped around the user, for someone who added
     * one in an OpenStreetMap app instead of the add form. */
    fun lookAgainNearby() {
        if (lookingNearby) return
        lookingNearby = true
        viewModelScope.launch {
            try {
                val fix = DeviceLocation.current(getApplication())
                if (fix == null) {
                    messageChannel.trySend("Could not determine your location. Try again outside, or with location turned on.")
                } else {
                    gpsFix = Triple(fix.latitude, fix.longitude, System.currentTimeMillis())
                    val (added, matched) = api.refreshNearby(fix.latitude, fix.longitude)
                    if (added.isEmpty() && matched == 0) {
                        messageChannel.trySend("Nothing new within 150 m yet. A place just added on OpenStreetMap can take a minute or two to show up.")
                    } else {
                        if (added.isNotEmpty()) messageChannel.trySend("Found ${added.joinToString { it.name }}: on the map for everyone now.")
                        // New countries, and any of the user's additions they confirmed.
                        refresh()
                        // The open country's list, with the places just found.
                        selected?.let { select(it) }
                    }
                }
            } catch (e: Exception) {
                messageChannel.trySend("Could not look again: ${e.message}")
            }
            lookingNearby = false
        }
    }

    /** True while a new place is being added. */
    var addingPlace by mutableStateOf(false); private set

    /** Where the add form's pin starts: the device's position, or the area's centre without one. */
    suspend fun startingPoint(): Pair<Double, Double> {
        DeviceLocation.current(getApplication())?.let { fix ->
            gpsFix = Triple(fix.latitude, fix.longitude, System.currentTimeMillis())
            return fix.latitude to fix.longitude
        }
        return userPosition() ?: ((location?.lat ?: 0.0) to (location?.lng ?: 0.0))
    }

    /**
     * Adds a restaurant OpenStreetMap lacks, where the user put the pin. Opens it once added.
     * `note = false` when the user adds it to OSM themselves, so no note asks mappers to.
     */
    fun addPlace(name: String, country: String, lat: Double, lng: Double, address: String?, website: String?, note: Boolean, onAdded: () -> Unit) {
        if (addingPlace) return
        addingPlace = true
        viewModelScope.launch {
            try {
                val (place, reward) = api.addPlace(name, country, lat, lng, address, website, note)
                val earns = reward.pendingText(country.uppercase(), catalog)
                messageChannel.trySend(
                    if (note) "${place.name} added. You see it right away. $earns"
                    else "${place.name} added for you. Now add it to the map in the other app, so everyone gets it. $earns",
                )
                onAdded()
                openPlace(place)
                // It counts towards its country, for this user, if it lies in the area.
                // Applied only if no refresh (such as a change of area) started meanwhile.
                val refresh = refreshes
                location?.let { here ->
                    runCatching { api.discover(here.area.id) }.onSuccess { if (refresh == refreshes) discover = it }
                }
            } catch (e: Exception) {
                messageChannel.trySend("Could not add the restaurant: ${e.message}")
            }
            addingPlace = false
        }
    }

    /** "Lalibela is on the map. +130 XP…" for each addition OpenStreetMap now has that the
     * user wasn't told about. Only those queued are marked seen; the rest wait for the next refresh. */
    private suspend fun announceConfirmedAdditions() {
        // A few at a time: any more wait for the next refresh.
        val news = runCatching { api.additions().filter { it.news }.take(10) }.getOrNull() ?: return
        val told = mutableListOf<String>()
        try {
            // `send` waits for room, where `trySend` would drop messages past the buffer.
            for (addition in news) {
                messageChannel.send(addition.announcement(catalog))
                told += addition.id
            }
        } finally {
            if (told.isNotEmpty()) withContext(NonCancellable) { runCatching { api.markAdditionsSeen(told) } }
        }
    }

    /** Updates a place's visits on its card (which shows only the latest) and in its details. */
    private fun setVisits(id: String, update: (List<Visit>) -> List<Visit>) {
        places = places.map { if (it.id == id) it.copy(visits = update(it.visits).take(1)) else it }
        detail = detail?.let { if (it.id == id) it.copy(visits = update(it.visits)) else it }
    }

    /** Announces everything a check-in unlocked by comparing progress before and after. */
    private fun celebrate(before: Discover, after: Discover, place: Place) {
        for (code in place.countries) {
            val was = before.countries.firstOrNull { it.code == code } ?: continue
            val now = after.countries.firstOrNull { it.code == code } ?: continue
            val info = country(code) ?: continue
            if (now.status == was.status || now.status == Status.AVAILABLE) continue
            val more = if (now.status == Status.COMPLETED) "" else " ${now.goal - now.visited} more for diamond."
            messageChannel.trySend("${info.flag} ${info.name}: ${now.status.label.lowercase()}.$more")
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

    fun loadAccount() {
        viewModelScope.launch {
            try {
                account = api.account()
                accountError = null
            } catch (e: Exception) {
                accountError = e.message
            }
        }
    }

    /** Runs one account action at a time, reporting failures in the snackbar. */
    private fun accountAction(work: suspend () -> Unit) {
        if (accountBusy) return
        accountBusy = true
        viewModelScope.launch {
            try {
                work()
            } catch (e: Exception) {
                messageChannel.trySend("That did not work: ${e.message}")
            }
            accountBusy = false
        }
    }

    fun makeRecoveryCode() = accountAction {
        recoveryCode = api.newRecoveryCode()
        account = api.account()
    }

    fun makeLinkCode() = accountAction { linkCode = api.newLinkCode() }

    /** Whether a share sheet is being prepared. */
    var sharing by mutableStateOf(false); private set

    /** Shares the map of the current area through a public page, made on first use. */
    fun shareProgress() {
        val areaId = location?.area?.id ?: return
        share { api.shareProgress(areaId) }
    }

    fun shareVisit(visitId: Long) = share { api.shareVisit(visitId) }

    private fun share(make: suspend () -> ShareLink) {
        if (sharing) return
        sharing = true
        viewModelScope.launch {
            try {
                val link = make()
                // The settings load the account, public link included, when they open.
                shareLink(getApplication(), link, api.card(link))
            } catch (e: Exception) {
                messageChannel.trySend("Could not share: ${e.message}")
            }
            sharing = false
        }
    }

    /** The public page as a full link; null when it's off. */
    val publicProfileUrl: String? get() = account?.publicProfile?.let { BuildConfig.API_BASE.trimEnd('/') + it }

    fun unshareProgress() = accountAction {
        api.unshareProgress()
        account = api.account()
    }

    fun clearLinkCode() {
        linkCode = null
    }

    /** Signs in to the account of a recovery or link code; what this device did moves into it. */
    fun restore(code: String, onRestored: () -> Unit) = accountAction {
        try {
            api.restore(code)
        } catch (e: ApiException) {
            if (e.status != 404) throw e
            messageChannel.trySend("Code not recognised. A link code works once, for 10 minutes.")
            return@accountAction
        }
        recoveryCode = null
        linkCode = null
        onRestored()
        messageChannel.trySend("Welcome back. Your progress is on this device now.")
        accountChanged()
    }

    fun signOut(deviceId: String) = accountAction {
        api.removeDevice(deviceId)
        account = api.account()
    }

    /** The address a sign-in code went to, while the code is awaited. */
    var codeSentTo by mutableStateOf<String?>(null); private set

    fun sendEmailCode(email: String) = accountAction {
        val address = email.trim()
        api.sendEmailCode(address)
        codeSentTo = address
    }

    fun forgetEmailCode() {
        codeSentTo = null
    }

    fun signIn(code: String) {
        val email = codeSentTo ?: return
        accountAction {
            try {
                api.signIn(email, code)
            } catch (e: ApiException) {
                if (e.status != 404) throw e
                messageChannel.trySend("Wrong code. Check it, or ask for a new one.")
                return@accountAction
            }
            codeSentTo = null
            recoveryCode = null
            linkCode = null
            messageChannel.trySend("Signed in. Your progress now follows you to every device you sign in on.")
            accountChanged()
        }
    }

    fun signOut() = accountAction {
        api.signOut()
        recoveryCode = null
        linkCode = null
        messageChannel.trySend("Signed out. Sign in again to get your progress back here.")
        accountChanged()
    }

    fun deleteAccount() = accountAction {
        api.deleteAccount()
        recoveryCode = null
        linkCode = null
        messageChannel.trySend("Account deleted. You start fresh on this device.")
        accountChanged()
    }

    /** This device is in another account now: drop what was loaded for the previous one. */
    private fun accountChanged() {
        closePlace()
        select(selected)
        refresh()
        account = null
        loadAccount()
    }
}

enum class DetailVisits { LOADING, LOADED, FAILED }
