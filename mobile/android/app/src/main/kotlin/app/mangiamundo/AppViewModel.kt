package app.mangiamundo

import android.app.Application
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import app.mangiamundo.core.Account
import app.mangiamundo.core.ApiClient
import app.mangiamundo.core.UnknownPlace
import app.mangiamundo.core.LinkCode
import app.mangiamundo.core.Camera
import app.mangiamundo.core.Catalog
import app.mangiamundo.core.Celebration
import app.mangiamundo.core.Country
import app.mangiamundo.core.Discover
import app.mangiamundo.core.Area
import app.mangiamundo.core.AreaProgress
import app.mangiamundo.core.Place
import app.mangiamundo.core.ProjectionKind
import app.mangiamundo.core.World
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class AppViewModel(application: Application) : AndroidViewModel(application) {
    internal val settings = Settings(application)
    internal val api = ApiClient(BuildConfig.API_BASE, settings, android.os.Build.MODEL) { accounts.signedOut() }
    private val accounts = AccountModel(this)
    internal val spins = SpinModel(this)
    private val visits = VisitsModel(this)
    private val osm = OsmRefreshModel(this)
    internal val adding = AddPlaceModel(this)
    private val locations = LocationModel(this)
    internal val reminders = RemindersModel(this)
    private val shares = ShareModel(this)
    private val loader = AreaLoader(this)
    private val browsing = PlacesModel(this)

    var world by mutableStateOf<World?>(null); private set
    var catalog by mutableStateOf<Catalog?>(null); internal set
    var location by mutableStateOf(settings.location); internal set
    /** Areas chosen recently, newest first, for the location picker. Includes the current one. */
    val recentAreas: List<Area> get() = locations.recentAreas
    var discover by mutableStateOf<Discover?>(null); internal set
    val loading: Boolean get() = loader.loading
    val error: String? get() = loader.error
    /** How far the server is with the area while it loads; null when there is nothing to show. */
    val areaProgress: AreaProgress? get() = loader.areaProgress
    /** Counts refreshes, so only the latest one shows or clears the progress wheel. */
    internal val refreshes: Int get() = loader.refreshes
    /** The first data is in (or failed), so the startup loader can go. Read by MainActivity. */
    @Volatile var ready = false; internal set

    /** Code of the country whose places are shown. */
    val selected: String? get() = browsing.selected
    var places by mutableStateOf<List<Place>>(emptyList()); internal set
    val placesLoading: Boolean get() = browsing.placesLoading
    /** The place a check-in, undo or report is running for. One at a time. */
    val busyPlace: String? get() = visits.busyPlace
    /** The place whose details are open: its card's data until the full place has loaded. */
    var detail by mutableStateOf<Place?>(null); internal set
    /** Whether the details hold all visits yet. Until then they show the card's latest one. */
    val detailVisits: DetailVisits get() = browsing.detailVisits

    /** A spin is running: the globe turns and the Spin buttons wait. */
    val spinning: Boolean get() = spins.spinning
    /** The place the last spin landed on, whose details offer to spin again. */
    val spun: String? get() = spins.spun

    val locating: Boolean get() = locations.locating
    val locateError: String? get() = locations.locateError

    var celebrations by mutableStateOf<List<Celebration>>(emptyList()); internal set
    val remindersOn: Boolean get() = reminders.remindersOn
    /** 0 = countries, 1 = badges. */
    var tab by mutableStateOf(0)
    /** The server's version and commit; null until it answers, "not reached" after a failure. */
    val server: String? get() = loader.server

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

    init {
        location?.let { camera = camera.copy(lng = it.lng, lat = it.lat) }
        loader.loadServer()
        viewModelScope.launch {
            world = withContext(Dispatchers.IO) {
                World.parse(application.assets.open("world.json").bufferedReader().use { it.readText() })
            }
        }
        loader.loadCatalog()
    }

    fun country(code: String): Country? = catalog?.countries?.firstOrNull { it.code == code }

    fun changeProjection(kind: ProjectionKind) {
        projection = kind
        settings.projection = kind
    }

    /** Picks a city or postal code from a search. */
    fun chooseArea(area: Area) = locations.chooseArea(area)
    /** Switches between the city and the postal code found at the GPS position. */
    fun switchArea(area: Area) = locations.switchArea(area)
    /** Takes an area off the recent list. */
    fun forgetRecent(id: String) = locations.forgetRecent(id)

    /** Where the user is, when known recently: distances are measured from it. */
    internal fun userPosition(): Pair<Double, Double>? {
        gpsFix?.takeIf { System.currentTimeMillis() - it.third < 10 * 60_000 }?.let { return it.first to it.second }
        return location?.takeIf { it.fromGps }?.let { it.lat to it.lng }
    }

    /** Moves to the device position. With `quiet`, failures leave the current location alone. */
    fun locate(quiet: Boolean = false) = locations.locate(quiet)
    fun locationDenied() = locations.locationDenied()

    suspend fun search(query: String): List<Area> = api.searchAreas(query)

    fun refresh() = loader.refresh()
    /** Opens what a notification points at. */
    fun openFromNotification(country: String?, badges: Boolean) = loader.openFromNotification(country, badges)

    /** Call after the notification permission was granted (or with false when refused). */
    fun setReminders(on: Boolean) = reminders.setReminders(on)

    fun select(code: String?) = browsing.select(code)
    /** Opens a place's details, with all of the user's visits. */
    fun openPlace(place: Place) = browsing.openPlace(place)
    fun closePlace() = browsing.closePlace()

    /** Spins the globe: it lands on a country and opens an unvisited place there, open now if any is. */
    fun spin() = spins.spin()
    /** "Not today": keeps the place out of spins for 20 hours, and spins again. */
    fun notToday(placeId: String) = spins.notToday(placeId)

    /** A tap on the map: open the country if it is available here, otherwise say so. */
    fun tapCountry(code: String, name: String) = browsing.tapCountry(code, name)

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
    val addingPlace: Boolean get() = adding.addingPlace
    /** Restaurants in the area with unknown food, nearest to the user first. */
    val unknownPlaces: List<UnknownPlace> get() = adding.unknownPlaces
    /** The unknown place whose answer is being saved. One at a time. */
    val answering: String? get() = adding.answering
    /** Where the add form's pin starts: the device's position, or the area's centre without one. */
    suspend fun startingPoint(): Pair<Double, Double> = adding.startingPoint()

    /** Adds a restaurant OpenStreetMap lacks, where the user put the pin. Opens it once added. */
    fun addPlace(name: String, country: String, lat: Double, lng: Double, address: String?, website: String?, note: Boolean, onAdded: () -> Unit) =
        adding.addPlace(name, country, lat, lng, address, website, note, onAdded)

    /** Loads [unknownPlaces] for the current area. A failure leaves the list empty. */
    fun loadUnknownPlaces() = adding.loadUnknownPlaces()
    /** "It serves food from …" (or, with a null `country`, "nothing specific"). */
    fun answerUnknown(unknown: UnknownPlace, country: String?, onAdded: () -> Unit) = adding.answerUnknown(unknown, country, onAdded)

    /** Whether a share sheet is being prepared. */
    val sharing: Boolean get() = shares.sharing
    /** Shares the map of the current area through a public page, made on first use. */
    fun shareProgress() = shares.shareProgress()
    fun shareVisit(visitId: Long) = shares.shareVisit(visitId)

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
