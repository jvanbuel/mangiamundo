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
import app.mangiamundo.core.UnknownPlace
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
import app.mangiamundo.core.pendingText
import app.mangiamundo.core.ProjectionKind
import app.mangiamundo.core.ReminderSchedule
import app.mangiamundo.core.World
import app.mangiamundo.core.withRecent
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlin.math.abs

private const val NOT_REACHED = "not reached"

class AppViewModel(application: Application) : AndroidViewModel(application) {
    internal val settings = Settings(application)
    internal val api = ApiClient(BuildConfig.API_BASE, settings, android.os.Build.MODEL) { accounts.signedOut() }
    private val accounts = AccountModel(this)
    private val spins = SpinModel(this)
    private val visits = VisitsModel(this)
    private val osm = OsmRefreshModel(this)

    var world by mutableStateOf<World?>(null); private set
    var catalog by mutableStateOf<Catalog?>(null); private set
    var location by mutableStateOf(settings.location); private set
    /** Areas chosen recently, newest first, for the location picker. Includes the current one. */
    var recentAreas by mutableStateOf(settings.recentAreas); private set
    var discover by mutableStateOf<Discover?>(null); internal set
    var loading by mutableStateOf(false); private set
    var error by mutableStateOf<String?>(null); private set
    /** How far the server is with the area while it loads; null when there is nothing to show. */
    var areaProgress by mutableStateOf<AreaProgress?>(null); private set
    /** Counts refreshes, so only the latest one shows or clears the progress wheel. */
    internal var refreshes = 0
        private set
    /** The first data is in (or failed), so the startup loader can go. Read by MainActivity. */
    @Volatile var ready = false; private set

    /** Code of the country whose places are shown. */
    var selected by mutableStateOf<String?>(null); private set
    var places by mutableStateOf<List<Place>>(emptyList()); internal set
    var placesLoading by mutableStateOf(false); private set
    /** The place a check-in, undo or report is running for. One at a time. */
    val busyPlace: String? get() = visits.busyPlace
    /** The place whose details are open: its card's data until the full place has loaded. */
    var detail by mutableStateOf<Place?>(null); internal set
    /** Whether the details hold all visits yet. Until then they show the card's latest one. */
    var detailVisits by mutableStateOf(DetailVisits.LOADING); private set
    private var detailJob: Job? = null

    /** A spin is running: the globe turns and the Spin buttons wait. */
    val spinning: Boolean get() = spins.spinning
    /** The place the last spin landed on, whose details offer to spin again. */
    val spun: String? get() = spins.spun

    var locating by mutableStateOf(false); private set
    var locateError by mutableStateOf<String?>(null); private set

    var celebrations by mutableStateOf<List<Celebration>>(emptyList()); private set
    var remindersOn by mutableStateOf(settings.remindersOn && Notifications.allowed(application)); private set
    /** 0 = countries, 1 = badges. */
    var tab by mutableStateOf(0)
    /** The server's version and commit; null until it answers, "not reached" after a failure. */
    var server by mutableStateOf<String?>(null); private set

    /** The account's recovery code status and devices, loaded when settings open. */
    val account: Account? get() = accounts.account
    val accountError: String? get() = accounts.accountError
    val accountBusy: Boolean get() = accounts.accountBusy
    /** Shown once, right after it is made. */
    val recoveryCode: String? get() = accounts.recoveryCode
    val linkCode: LinkCode? get() = accounts.linkCode
    /** The address a sign-in code went to, while the code is awaited. */
    val codeSentTo: String? get() = accounts.codeSentTo
    /** The public page as a full link; null when it's off. */
    val publicProfileUrl: String? get() = accounts.publicProfileUrl

    var projection by mutableStateOf(settings.projection); private set
    var camera by mutableStateOf(Camera())

    internal val messageChannel = Channel<String>(Channel.BUFFERED)
    /** Short announcements (badges, discoveries, errors) for the snackbar. */
    val messages = messageChannel.receiveAsFlow()

    /** The last real device position, used to verify check-ins while browsing another city. */
    internal var gpsFix: Triple<Double, Double, Long>? = null
    private var refreshJob: Job? = null
    private var remindersJob: Job? = null
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

    /** Where the user is, when known recently: distances are measured from it. */
    internal fun userPosition(): Pair<Double, Double>? {
        gpsFix?.takeIf { System.currentTimeMillis() - it.third < 10 * 60_000 }?.let { return it.first to it.second }
        return location?.takeIf { it.fromGps }?.let { it.lat to it.lng }
    }

    private fun moveTo(target: SavedLocation) {
        // The area being left too, so one saved before recents existed shows up.
        recentAreas = withRecent(location?.let { withRecent(recentAreas, it.area) } ?: recentAreas, target.area)
        settings.recentAreas = recentAreas
        location = target
        spins.forgetSpun()
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

    /** Schedules what the server plans for the area. Without an answer, what was scheduled stays. */
    private fun rescheduleReminders() {
        if (!remindersOn) return
        val here = location ?: return
        remindersJob?.cancel()
        remindersJob = viewModelScope.launch {
            val plan = try {
                api.reminders(here.area.id)
            } catch (e: kotlinx.coroutines.CancellationException) {
                throw e
            } catch (e: Exception) {
                return@launch
            }
            // Turned off or moved meanwhile: this plan is no longer wanted.
            if (!remindersOn || location?.area?.id != here.area.id) return@launch
            Notifications.schedule(getApplication(), ReminderSchedule.requests(plan, java.time.ZonedDateTime.now()))
        }
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
    fun spin() = spins.spin()

    /** "Not today": keeps the place out of spins for 20 hours, and spins again. */
    fun notToday(placeId: String) = spins.notToday(placeId)

    /** A tap on the map: open the country if it is available here, otherwise say so. */
    fun tapCountry(code: String, name: String) {
        if (discover?.countries?.any { it.code == code } == true) {
            select(code)
        } else if (discover != null) {
            messageChannel.trySend("No places from $name in ${location?.area?.name ?: "this area"}.")
        }
    }

    fun toggleVisit(place: Place) = visits.toggleVisit(place)

    /** Reports a place as closed for good: it disappears from this user's lists right away. */
    fun reportClosed(place: Place) = visits.reportClosed(place)

    /** The place being read again from OpenStreetMap. */
    val refreshingPlace: String? get() = osm.refreshingPlace
    /** True while OpenStreetMap is asked for places just mapped nearby. */
    val lookingNearby: Boolean get() = osm.lookingNearby

    /** The user went to edit the place in an OpenStreetMap app: read it again when they come back. */
    fun editingOnOsm(placeId: String) = osm.editingOnOsm(placeId)

    /** Back in the app: shows an edit just made to a place on OpenStreetMap. */
    fun onResume() = osm.onResume()

    /** Reads a place again from OpenStreetMap and says what changed. With `quiet`, a failure isn't shown. */
    fun refreshPlace(placeId: String, quiet: Boolean = false) = osm.refreshPlace(placeId, quiet)

    /** Asks OpenStreetMap for restaurants just mapped around the user. */
    fun lookAgainNearby() = osm.lookAgainNearby()

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
        val refresh = refreshes
        location?.let { here ->
            runCatching { api.discover(here.area.id) }.onSuccess { if (refresh == refreshes) discover = it }
        }
    }

    /** Restaurants in the area with unknown food, nearest to the user first. */
    var unknownPlaces by mutableStateOf<List<UnknownPlace>>(emptyList()); private set
    /** The unknown place whose answer is being saved. One at a time. */
    var answering by mutableStateOf<String?>(null); private set

    /** Loads [unknownPlaces] for the current area. A failure leaves the list empty. */
    fun loadUnknownPlaces() {
        val here = location ?: return
        viewModelScope.launch {
            val found = runCatching { api.unknownPlaces(here.area.id, userPosition()) }.getOrDefault(emptyList())
            if (location?.area?.id == here.area.id) unknownPlaces = found
        }
    }

    /** "It serves food from …" (or, with a null `country`, "nothing specific") for a restaurant
     * with unknown food. A suggestion opens the place it adds, after `onAdded`. */
    fun answerUnknown(unknown: UnknownPlace, country: String?, onAdded: () -> Unit) {
        if (answering != null) return
        answering = unknown.id
        viewModelScope.launch {
            try {
                if (country == null) {
                    api.noCuisine(unknown.id)
                } else {
                    val (place, reward) = api.suggestCuisine(unknown.id, country)
                    messageChannel.trySend("${place.name} added. You see it right away. ${reward.pendingText(country.uppercase(), catalog)}")
                    onAdded()
                    openPlace(place)
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

    fun loadAccount() = accounts.loadAccount()
    fun makeRecoveryCode() = accounts.makeRecoveryCode()
    fun makeLinkCode() = accounts.makeLinkCode()
    fun unshareProgress() = accounts.unshareProgress()
    fun clearLinkCode() = accounts.clearLinkCode()
    /** Signs in to the account of a recovery or link code; what this device did moves into it. */
    fun restore(code: String, onRestored: () -> Unit) = accounts.restore(code, onRestored)
    fun signOut(deviceId: String) = accounts.signOut(deviceId)
    fun sendEmailCode(email: String) = accounts.sendEmailCode(email)
    fun forgetEmailCode() = accounts.forgetEmailCode()
    fun signIn(code: String) = accounts.signIn(code)
    /** Signs in with the ID token `getToken` gets from Google's account picker. */
    fun signInWithGoogle(getToken: suspend () -> String?) = accounts.signInWithGoogle(getToken)
    fun signOut() = accounts.signOut()
    fun deleteAccount() = accounts.deleteAccount()

    /** Goes up with every change of account, so an answer for the previous one can be told apart. */
    internal var accountGeneration = 0
        private set

    /** This device is in another account now: drop what was loaded for the previous one. */
    internal fun accountChanged() {
        accountGeneration++
        closePlace()
        select(selected)
        refresh()
        accounts.reload()
    }
}

enum class DetailVisits { LOADING, LOADED, FAILED }
