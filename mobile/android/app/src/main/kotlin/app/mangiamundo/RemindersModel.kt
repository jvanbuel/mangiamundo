package app.mangiamundo

import android.app.Application
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.viewModelScope
import app.mangiamundo.core.ReminderSchedule
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch

/** Reminders of what the area has to celebrate, as local notifications. Part of [AppViewModel]. */
internal class RemindersModel(private val vm: AppViewModel) {
    private val api get() = vm.api
    private val messageChannel get() = vm.messageChannel
    private val settings get() = vm.settings

    var remindersOn by mutableStateOf(settings.remindersOn && Notifications.allowed(vm.getApplication())); private set
    private var remindersJob: Job? = null

    /** Call after the notification permission was granted (or with false when refused). */
    fun setReminders(on: Boolean) {
        val app = vm.getApplication<Application>()
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
    fun rescheduleReminders() {
        if (!remindersOn) return
        val here = vm.location ?: return
        remindersJob?.cancel()
        remindersJob = vm.viewModelScope.launch {
            val plan = try {
                api.reminders(here.area.id)
            } catch (e: kotlinx.coroutines.CancellationException) {
                throw e
            } catch (e: Exception) {
                return@launch
            }
            // Turned off or moved meanwhile: this plan is no longer wanted.
            if (!remindersOn || vm.location?.area?.id != here.area.id) return@launch
            Notifications.schedule(vm.getApplication(), ReminderSchedule.requests(plan, java.time.ZonedDateTime.now()))
        }
    }
}
