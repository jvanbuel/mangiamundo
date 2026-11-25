package app.mangiamundo

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.viewModelScope
import app.mangiamundo.core.Account
import app.mangiamundo.core.ApiException
import app.mangiamundo.core.LinkCode
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch

/** The account in the settings: its codes, sign-in and devices. Part of [AppViewModel]. */
internal class AccountModel(private val vm: AppViewModel) {
    private val api get() = vm.api
    private val messageChannel get() = vm.messageChannel

    /** The account's recovery code status and devices, loaded when settings open. */
    var account by mutableStateOf<Account?>(null); private set
    var accountError by mutableStateOf<String?>(null); private set
    var accountBusy by mutableStateOf(false); private set
    /** Shown once, right after it is made. */
    var recoveryCode by mutableStateOf<String?>(null); private set
    var linkCode by mutableStateOf<LinkCode?>(null); private set

    /** The address a sign-in code went to, while the code is awaited. */
    var codeSentTo by mutableStateOf<String?>(null); private set

    fun loadAccount() {
        vm.viewModelScope.launch {
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
        vm.viewModelScope.launch {
            try {
                work()
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                messageChannel.trySend("That did not work: ${e.message}")
            } finally {
                accountBusy = false
            }
        }
    }

    fun makeRecoveryCode() = accountAction {
        recoveryCode = api.newRecoveryCode()
        account = api.account()
    }

    fun makeLinkCode() = accountAction { linkCode = api.newLinkCode() }

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
        vm.accountChanged()
    }

    fun signOut(deviceId: String) = accountAction {
        api.removeDevice(deviceId)
        account = api.account()
    }

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
            vm.accountChanged()
        }
    }

    /**
     * Signs in with the ID token `getToken` gets from Google's account picker (null when the
     * user closes it). The picker counts as part of the account action, so no other one can
     * start meanwhile, and it runs in this model's scope, so leaving the screen doesn't cut it off.
     */
    fun signInWithGoogle(getToken: suspend () -> String?) = accountAction {
        val idToken = getToken() ?: return@accountAction
        api.signInWithGoogle(idToken)
        codeSentTo = null
        recoveryCode = null
        linkCode = null
        messageChannel.trySend("Signed in. Your progress now follows you to every device you sign in on.")
        vm.accountChanged()
    }

    fun signOut() = accountAction {
        api.signOut()
        recoveryCode = null
        linkCode = null
        messageChannel.trySend("Signed out. Sign in again to get your progress back here.")
        vm.accountChanged()
    }

    fun deleteAccount() = accountAction {
        api.deleteAccount()
        recoveryCode = null
        linkCode = null
        messageChannel.trySend("Account deleted. You start fresh on this device.")
        vm.accountChanged()
    }

    /**
     * The server no longer knows this device: it was signed out from another device, or its
     * account is gone. Says so, and shows everything again as for someone without an account,
     * so nothing of the old account stays on screen as if it were still here. A new account
     * only starts with the next check-in or other change.
     */
    fun signedOut() {
        // On its own: the request that found out is often part of what this starts again.
        vm.viewModelScope.launch {
            messageChannel.trySend("You were signed out. Sign in or use a recovery code to get your progress back.")
            recoveryCode = null
            linkCode = null
            vm.accountChanged()
        }
    }

    /** The device is in another account now: forget the previous one's and load this one. */
    fun reload() {
        account = null
        loadAccount()
    }
}
