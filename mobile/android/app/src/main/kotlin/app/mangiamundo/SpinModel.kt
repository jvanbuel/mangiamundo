package app.mangiamundo

import android.app.Application
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.viewModelScope
import app.mangiamundo.core.ApiException
import app.mangiamundo.core.Camera
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/** How long the globe turns at least before a spin lands, and how long one turn takes. */
private const val SPIN_MS = 1600L

/** Spins: the globe turns, lands on a country and opens a place there. Part of [AppViewModel]. */
internal class SpinModel(private val vm: AppViewModel) {
    private val api get() = vm.api
    private val messageChannel get() = vm.messageChannel

    /** A spin is running: the globe turns and the Spin buttons wait. */
    var spinning by mutableStateOf(false); private set
    /** The place the last spin landed on, whose details offer to spin again. */
    var spun by mutableStateOf<String?>(null); private set
    /** Places suggested since the area was picked, so spinning again finds another. */
    private var spunIds = mutableListOf<String>()

    /** A new area was picked: spinning may suggest every place again. */
    fun forgetSpun() {
        spunIds.clear()
    }

    /** Spins the globe: it lands on a country and opens an unvisited place there, open now if any is. */
    fun spin() {
        val here = vm.location ?: return
        if (spinning) return
        spinning = true
        vm.closePlace()
        vm.viewModelScope.launch {
            val still = animationsOff()
            val turning = if (still) null else launch { turn() }
            val started = System.currentTimeMillis()
            val exclude = spunIds + vm.settings.notToday.keys
            try {
                val found = try {
                    api.suggestion(here.area.id, vm.userPosition(), exclude, open = true)
                } catch (e: ApiException) {
                    if (e.code != "nothing_open") throw e
                    messageChannel.trySend("Nothing open right now. Here is one for another time.")
                    api.suggestion(here.area.id, vm.userPosition(), exclude, open = false)
                }
                if (!still) delay(SPIN_MS - (System.currentTimeMillis() - started))
                turning?.cancelAndJoin()
                if (vm.location != here) return@launch
                spunIds += found.place.id
                spun = found.place.id
                vm.select(found.country)
                vm.world?.countries?.firstOrNull { it.code == found.country }?.let { glide(it.landing(), still) }
                vm.openPlace(found.place)
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
        vm.settings.notToday = vm.settings.notToday + (placeId to System.currentTimeMillis() + 20 * 3_600_000L)
        spin()
    }

    /** Turns the globe one turn per SPIN_MS, zoomed out to all of it, until cancelled. */
    private suspend fun turn() {
        val from = vm.camera
        val started = System.nanoTime()
        while (true) {
            val ms = (System.nanoTime() - started) / 1e6
            val out = minOf(1.0, ms / 500)
            vm.camera = from.copy(lng = from.lng + 360 * ms / SPIN_MS, zoom = from.zoom + (1.0 - from.zoom) * out).normalized()
            delay(16)
        }
    }

    /** Moves the camera to `to`, slowing down at the end; the short way around the globe. */
    private suspend fun glide(to: Camera, still: Boolean) {
        val from = vm.camera
        if (still) {
            vm.camera = to
            return
        }
        val delta = ((to.lng - from.lng + 540) % 360) - 180
        val started = System.nanoTime()
        while (true) {
            val t = minOf(1.0, (System.nanoTime() - started) / 1e6 / 900)
            val e = 1 - (1 - t) * (1 - t) * (1 - t)
            vm.camera = Camera(from.lng + delta * e, from.lat + (to.lat - from.lat) * e, from.zoom + (to.zoom - from.zoom) * e).normalized()
            if (t >= 1) return
            delay(16)
        }
    }

    /** Animations turned off in the system settings: spins land straight away. */
    private fun animationsOff() = android.provider.Settings.Global.getFloat(
        vm.getApplication<Application>().contentResolver,
        android.provider.Settings.Global.ANIMATOR_DURATION_SCALE,
        1f,
    ) == 0f
}
