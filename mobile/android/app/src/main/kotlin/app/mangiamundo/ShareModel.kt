package app.mangiamundo

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.viewModelScope
import app.mangiamundo.core.ShareLink
import app.mangiamundo.ui.shareLink
import kotlinx.coroutines.launch

/** Public pages of the user's progress or of a visit, shared from the share sheet. Part of [AppViewModel]. */
internal class ShareModel(private val vm: AppViewModel) {
    private val api get() = vm.api

    /** Whether a share sheet is being prepared. */
    var sharing by mutableStateOf(false); private set

    /** Shares the map of the current area through a public page, made on first use. */
    fun shareProgress() {
        val areaId = vm.location?.area?.id ?: return
        share { api.shareProgress(areaId) }
    }

    fun shareVisit(visitId: Long) = share { api.shareVisit(visitId) }

    private fun share(make: suspend () -> ShareLink) {
        if (sharing) return
        sharing = true
        vm.viewModelScope.launch {
            try {
                val link = make()
                // The settings load the account, public link included, when they open.
                shareLink(vm.getApplication(), link, api.card(link))
            } catch (e: Exception) {
                vm.messageChannel.trySend("Could not share: ${e.message}")
            }
            sharing = false
        }
    }
}
