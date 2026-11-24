package app.mangiamundo.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.cash.paparazzi.DeviceConfig
import app.cash.paparazzi.Paparazzi
import app.mangiamundo.SavedLocation
import app.mangiamundo.core.Area
import app.mangiamundo.core.Badge
import app.mangiamundo.core.BadgeKind
import app.mangiamundo.core.Camera
import app.mangiamundo.core.Celebration
import app.mangiamundo.core.Country
import app.mangiamundo.core.EventProgress
import app.mangiamundo.core.EventStatus
import app.mangiamundo.core.NearbyCountry
import app.mangiamundo.core.Reminder
import java.time.LocalDate
import app.mangiamundo.core.CountryProgress
import app.mangiamundo.core.Status
import app.mangiamundo.core.Discover
import app.mangiamundo.core.GroupProgress
import app.mangiamundo.core.Place
import app.mangiamundo.core.Profile
import app.mangiamundo.core.Visit
import app.mangiamundo.core.Day
import app.mangiamundo.core.Hours
import app.mangiamundo.core.HoursStatus
import app.mangiamundo.core.ProjectionKind
import app.mangiamundo.core.World
import com.android.resources.NightMode
import org.junit.Rule
import org.junit.Test
import java.io.File
import java.util.Locale
import java.util.TimeZone

/** Renders the main screens with sample data, in light and dark mode. */
class ScreensTest {
    @get:Rule
    val paparazzi = Paparazzi(deviceConfig = DeviceConfig.PIXEL_6, maxPercentDifference = 0.1)

    // Dates are written, and "visited today" is decided, in the machine's language and time
    // zone. Pinned, so the images are the same wherever the tests run.
    init {
        Locale.setDefault(Locale.US)
        TimeZone.setDefault(TimeZone.getTimeZone("UTC"))
    }

    private val world = World.parse(File("../../../shared/world.json").readText())
    private val leuven = SavedLocation(Area("demo-leuven", "Leuven", lat = 50.8798, lng = 4.7005), 50.8798, 4.7005, fromGps = true)

    private val countries = listOf(
        Country("IT", "Italy", "EU", "🇮🇹"), Country("FR", "France", "EU", "🇫🇷"),
        Country("GR", "Greece", "EU", "🇬🇷"), Country("ET", "Ethiopia", "AF", "🇪🇹"),
        Country("MA", "Morocco", "AF", "🇲🇦"), Country("JP", "Japan", "AS", "🇯🇵"),
        Country("TH", "Thailand", "AS", "🇹🇭"), Country("MX", "Mexico", "NA", "🇲🇽"),
        Country("PE", "Peru", "SA", "🇵🇪"), Country("CN", "China", "AS", "🇨🇳"),
    )
    private val progress = listOf(
        CountryProgress("IT", "EU", 6, 6, 6, Status.COMPLETED), CountryProgress("FR", "EU", 4, 1, 4, Status.DISCOVERED),
        CountryProgress("GR", "EU", 3, 0, 3, Status.AVAILABLE), CountryProgress("ET", "AF", 2, 1, 2, Status.DISCOVERED),
        CountryProgress("MA", "AF", 2, 0, 2, Status.AVAILABLE), CountryProgress("JP", "AS", 14, 6, 10, Status.PLATINUM),
        CountryProgress("TH", "AS", 3, 0, 3, Status.AVAILABLE), CountryProgress("CN", "AS", 7, 0, 7, Status.AVAILABLE),
        CountryProgress("MX", "NA", 2, 0, 2, Status.AVAILABLE), CountryProgress("PE", "SA", 1, 0, 1, Status.AVAILABLE),
    )
    private val discover = Discover(
        countries = progress,
        continents = listOf(
            GroupProgress("AF", "Africa", 2, 1, 0), GroupProgress("AS", "Asia", 3, 1, 0),
            GroupProgress("EU", "Europe", 3, 2, 1), GroupProgress("NA", "North America", 1, 0, 0),
            GroupProgress("OC", "Oceania", 0, 0, 0), GroupProgress("SA", "South America", 1, 0, 0),
        ),
        world = GroupProgress("WORLD", "World", 10, 4, 1),
        places = 35, placesVisited = 10,
        badges = listOf(
            Badge("first-bite", BadgeKind.VISITS, "First check-in", "Check in at any place.", 1, 1, true),
            Badge("flags-5", BadgeKind.COUNTRIES, "5 countries", "Eat the food of 5 countries.", 4, 5, false),
            Badge("complete-1", BadgeKind.COMPLETED, "1 diamond country", "", 1, 1, true),
            Badge("continent-EU", BadgeKind.CONTINENT, "Europe", "", 2, 3, false),
            Badge("world", BadgeKind.WORLD, "Whole world", "", 4, 10, false),
        ),
        profile = Profile(xp = 300, level = 2, levelXp = 100, nextLevelXp = 400, visits = 10, countries = 4),
    )

    @Composable
    private fun Home(kind: ProjectionKind, camera: Camera) {
        val colors = LocalAppColors.current
        Column(Modifier.fillMaxSize().background(colors.panel)) {
            Box(Modifier.fillMaxWidth().weight(0.42f).background(colors.bg)) {
                WorldMap(
                    world = world,
                    statuses = progress.associate { it.code to it.status },
                    selected = "IT",
                    here = leuven.lng to leuven.lat,
                    kind = kind,
                    camera = camera,
                    onCamera = {},
                    onTap = {},
                    modifier = Modifier.fillMaxSize(),
                )
                Legend(Modifier.align(Alignment.BottomStart).padding(8.dp))
            }
            Column(Modifier.weight(0.58f).padding(16.dp)) {
                Header(leuven, onChange = {}, onSettings = {})
                Passport(discover)
                ContinentHeader(discover.continents[2])
                for (code in listOf("FR", "GR", "IT")) {
                    CountryRow(progress.first { it.code == code }, countries.first { it.code == code }) {}
                }
            }
        }
    }

    private fun snapshot(dark: Boolean, content: @Composable () -> Unit) {
        paparazzi.unsafeUpdateConfig(
            DeviceConfig.PIXEL_6.copy(nightMode = if (dark) NightMode.NIGHT else NightMode.NOTNIGHT),
        )
        paparazzi.snapshot { MangiamundoTheme { content() } }
    }

    @Test
    fun globeLight() = snapshot(dark = false) { Home(ProjectionKind.GLOBE, Camera(15.0, 35.0, 1.0)) }

    @Test
    fun globeDark() = snapshot(dark = true) { Home(ProjectionKind.GLOBE, Camera(15.0, 35.0, 1.0)) }

    @Test
    fun flatDark() = snapshot(dark = true) { Home(ProjectionKind.FLAT, Camera(10.0, 25.0, 1.0)) }

    @Test
    fun countryDetailLight() = snapshot(dark = false) {
        val colors = LocalAppColors.current
        Column(Modifier.fillMaxSize().background(colors.panel).padding(16.dp)) {
            CountryHeader(progress.first { it.code == "JP" }, countries.first { it.code == "JP" }) {}
            PlaceRow(Place("a", "Sushi Kaito", 50.88, 4.70, address = "Naamsestraat 12, Leuven", url = "https://example.org", countries = listOf("JP"), distanceM = 420, visits = listOf(Visit(1, 1, verified = true))), busy = false, onOpen = {}) {}
            PlaceRow(Place("b", "Ramen Bar Ichi", 50.87, 4.71, address = "Bondgenotenlaan 40", countries = listOf("JP"), distanceM = 1250, visits = listOf(Visit(2, 1))), busy = false, onOpen = {}) {}
            PlaceRow(Place("c", "Izakaya Kumo", 50.86, 4.69, countries = listOf("JP"), distanceM = 3400), busy = false, onOpen = {}) {}
            BadgeGrid(discover.badges)
        }
    }

    @Test
    fun placeDetailDark() = snapshot(dark = true) {
        val colors = LocalAppColors.current
        val open = listOf("11:30-14:00", "18:00-22:00")
        val week = listOf("Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday")
            .mapIndexed { i, name -> Day(name, today = i == 2, open = if (i == 0) emptyList() else if (i == 5) listOf("18:00-02:00") else open) }
        val place = Place(
            "osm-node-1", "Sushi Kaito", 50.88, 4.70, address = "Naamsestraat 12, Leuven", url = "https://example.org",
            cuisine = "japanese;sushi", hours = Hours("Tu-Su 11:30-14:00,18:00-22:00", HoursStatus(true, "Open until 14:00"), week),
            countries = listOf("JP"), distanceM = 420,
            visits = listOf(Visit(2, 1_794_830_400, verified = true), Visit(1, 1_790_000_000)),
            osmUrl = "https://www.openstreetmap.org/node/1",
        )
        Column(Modifier.fillMaxSize().background(colors.panel).padding(top = 16.dp)) {
            PlaceDetail(place, { code -> countries.firstOrNull { it.code == code } }, busy = false, today = today) {}
        }
    }

    private val today = LocalDate.of(2026, 11, 16)
    private fun day(offset: Long) = today.plusDays(offset).toString()

    private val celebrations = listOf(
        Celebration("thanksgiving", "Thanksgiving", "", listOf("US"), day(10), day(10), false, listOf(NearbyCountry("MX", 3)), Reminder("", "")),
        Celebration("lny", "Lunar New Year", "", listOf("CN", "JP"), day(40), day(42), false, listOf(NearbyCountry("CN", 7), NearbyCountry("JP", 5)), Reminder("", "")),
        Celebration("eid", "Eid al-Fitr", "", listOf("MA"), day(70), day(72), true, listOf(NearbyCountry("MA", 2)), Reminder("", "")),
    )
    private val events = listOf(
        EventProgress("levant", "Levant month", "Eat at 2 Lebanese, Syrian or Jordanian places in November.", listOf("LB"), day(-15), day(14), 2, 1, false, EventStatus.ACTIVE, 3),
        EventProgress("japan", "Japan month", "Eat at 3 Japanese places in December.", listOf("JP"), day(15), day(45), 3, 0, false, EventStatus.UPCOMING, 5),
        EventProgress("italy", "Italy week", "", listOf("IT"), day(-60), day(-53), 2, 2, true, EventStatus.ENDED, 6),
    )

    @Test
    fun comingUpAndLimitedTimeDark() = snapshot(dark = true) {
        val colors = LocalAppColors.current
        Column(Modifier.fillMaxSize().background(colors.panel).padding(16.dp)) {
            ComingUp(celebrations, remindersOn = false, onToggleReminders = {}, country = { code -> countries.firstOrNull { it.code == code } }, onOpen = {}, today = today)
            BadgeGrid(discover.badges.take(2), events, today = today)
        }
    }
}
