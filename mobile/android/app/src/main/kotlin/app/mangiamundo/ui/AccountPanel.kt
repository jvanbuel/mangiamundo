package app.mangiamundo.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.ClipEntry
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.mangiamundo.AppViewModel
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/** Recovery code, link code, signed-in devices and account deletion, in the settings sheet. */
@Composable
fun AccountSection(vm: AppViewModel) {
    val colors = LocalAppColors.current
    val uriHandler = LocalUriHandler.current
    val small = MaterialTheme.typography.bodySmall
    val clipboard = LocalClipboard.current
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    val copy = { text: String ->
        scope.launch { clipboard.setClipEntry(ClipEntry(android.content.ClipData.newPlainText("Mangiamundo code", text))) }
        Unit
    }
    var entered by remember { mutableStateOf("") }
    var email by remember { mutableStateOf("") }
    var signInCode by remember { mutableStateOf("") }
    var confirm by remember { mutableStateOf<Confirm?>(null) }
    var now by remember { mutableLongStateOf(System.currentTimeMillis() / 1000) }

    LaunchedEffect(Unit) { vm.loadAccount() }
    LaunchedEffect(vm.linkCode) {
        while (vm.linkCode != null) {
            now = System.currentTimeMillis() / 1000
            if (vm.linkCode!!.expiresAt <= now) vm.clearLinkCode()
            delay(5_000)
        }
    }

    confirm?.let { what ->
        AlertDialog(
            onDismissRequest = { confirm = null },
            title = { Text(what.title) },
            text = { Text(what.text) },
            confirmButton = {
                TextButton(onClick = {
                    confirm = null
                    when (what) {
                        Confirm.NEW_RECOVERY_CODE -> vm.makeRecoveryCode()
                        Confirm.DELETE -> vm.deleteAccount()
                        Confirm.SIGN_OUT -> vm.signOut()
                    }
                }) { Text(what.action) }
            },
            dismissButton = { TextButton(onClick = { confirm = null }) { Text("Cancel") } },
        )
    }

    Column(Modifier.padding(top = 16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        HorizontalDivider(color = colors.line)
        Heading("Sign in")
        val identities = vm.account?.identities.orEmpty()
        if (identities.isNotEmpty()) {
            val names = mapOf("apple" to "Apple", "google" to "Google", "email" to "email")
            val list = identities.joinToString { i -> (names[i.provider] ?: i.provider) + (i.email?.let { " ($it)" } ?: "") }
            Text("Signed in with $list.", color = colors.muted, style = small)
            ChunkyButton("Sign out of this device", onClick = { confirm = Confirm.SIGN_OUT }, enabled = !vm.accountBusy)
        } else {
            Text(
                "Optional. Sign in to keep your progress on all your devices; without it, it stays on this one.",
                color = colors.muted,
                style = small,
            )
            ChunkyButton(
                "Continue with Google",
                onClick = { vm.signInWithGoogle { googleIdToken(context) } },
                enabled = !vm.accountBusy,
                face = Color.White,
                content = Color(0xFF1F1F1F),
            )
            val sentTo = vm.codeSentTo
            if (sentTo != null) {
                Text("Enter the code we sent to $sentTo.", color = colors.muted, style = small)
                val submitCode = { if (signInCode.isNotBlank()) { vm.signIn(signInCode); signInCode = "" } }
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedTextField(
                        value = signInCode,
                        onValueChange = { signInCode = it },
                        placeholder = { Text("123456") },
                        singleLine = true,
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number, imeAction = ImeAction.Done),
                        keyboardActions = KeyboardActions(onDone = { submitCode() }),
                        modifier = Modifier.weight(1f),
                    )
                    ChunkyButton("Sign in", onClick = submitCode, enabled = !vm.accountBusy && signInCode.isNotBlank())
                }
                TextButton(onClick = vm::forgetEmailCode) { Text("Use another address") }
            } else {
                Text("Get a code by email.", color = colors.muted, style = small)
                val send = { if (email.isNotBlank()) vm.sendEmailCode(email) }
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedTextField(
                        value = email,
                        onValueChange = { email = it },
                        placeholder = { Text("you@example.com") },
                        singleLine = true,
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email, autoCorrectEnabled = false, imeAction = ImeAction.Send),
                        keyboardActions = KeyboardActions(onSend = { send() }),
                        modifier = Modifier.weight(1f),
                    )
                    ChunkyButton("Send code", onClick = send, enabled = !vm.accountBusy && email.isNotBlank())
                }
            }
        }

        Heading("Your progress")
        Text(
            if (identities.isNotEmpty()) "A recovery code also gets it back, without signing in."
            else "It lives on this device. Keep a recovery code to get it back after reinstalling the app or on a new phone.",
            color = colors.muted,
            style = small,
        )
        val code = vm.recoveryCode
        if (code != null) {
            ShownCode(code) { copy(code) }
            Text("Shown only now. Anyone with this code can use your account.", color = colors.muted, style = small)
        } else {
            // Until the account has loaded, it is unknown whether a code would replace one.
            val loaded = vm.account != null
            val made = vm.account?.recoveryCodeCreatedAt
            ChunkyButton(
                if (made != null) "Make a new recovery code" else "Make a recovery code",
                onClick = { if (made != null) confirm = Confirm.NEW_RECOVERY_CODE else vm.makeRecoveryCode() },
                enabled = !vm.accountBusy && loaded,
            )
            if (made != null) Text("You made one ${ago(made, now)}.", color = colors.muted, style = small)
        }

        vm.publicProfileUrl?.let { url ->
            Heading("Public link")
            Text(
                "Anyone with your link can see your map, level and badges. Turning it off breaks the link; sharing again makes a new one.",
                color = colors.muted,
                style = small,
            )
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                ChunkyButton("Open", onClick = { uriHandler.openUri(url) }, face = colors.panelRaised, content = colors.text)
                ChunkyButton("Turn off", onClick = vm::unshareProgress, enabled = !vm.accountBusy && !vm.sharing, face = colors.panelRaised, content = colors.text)
            }
        }

        Heading("Use it on another device")
        val link = vm.linkCode
        if (link != null) {
            ShownCode(link.code) { copy(link.code) }
            val minutes = ((link.expiresAt - now + 59) / 60).coerceAtLeast(0)
            Text(
                "Enter it there under Settings within $minutes ${if (minutes == 1L) "minute" else "minutes"}. It works once.",
                color = colors.muted,
                style = small,
            )
        } else {
            ChunkyButton("Show a link code", onClick = vm::makeLinkCode, enabled = !vm.accountBusy)
        }

        Heading("Have a recovery or link code?")
        val submit = { if (entered.isNotBlank()) vm.restore(entered) { entered = "" } }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedTextField(
                value = entered,
                onValueChange = { entered = it },
                placeholder = { Text("XXXX-XXXX") },
                singleLine = true,
                keyboardOptions = KeyboardOptions(
                    capitalization = KeyboardCapitalization.Characters,
                    autoCorrectEnabled = false,
                    imeAction = ImeAction.Done,
                ),
                keyboardActions = KeyboardActions(onDone = { submit() }),
                modifier = Modifier.weight(1f),
            )
            ChunkyButton("Use", onClick = submit, enabled = !vm.accountBusy && entered.isNotBlank())
        }
        Text("What you did on this device so far is added to that account.", color = colors.muted, style = small)

        val devices = vm.account?.devices.orEmpty()
        vm.accountError?.let {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(it, color = colors.muted, style = small, modifier = Modifier.weight(1f))
                TextButton(onClick = vm::loadAccount) { Text("Try again") }
            }
        }
        if (devices.size > 1) {
            Heading("Signed-in devices")
            for (device in devices) {
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Text(device.name ?: "Unnamed device", color = colors.text, fontWeight = FontWeight.Bold)
                        Text(
                            if (device.current) "This device" else "Used ${ago(device.lastUsedAt, now)}",
                            color = colors.muted,
                            style = small,
                        )
                    }
                    if (!device.current) {
                        TextButton(onClick = { vm.signOut(device.id) }, enabled = !vm.accountBusy) { Text("Sign out") }
                    }
                }
            }
        }

        // A device that only looked around has no account yet: there is nothing to delete.
        if (vm.account?.devices?.isEmpty() != true) {
            Text(
                "Delete my account",
                color = colors.muted,
                textDecoration = TextDecoration.Underline,
                modifier = Modifier.padding(top = 8.dp).clickable(enabled = !vm.accountBusy) { confirm = Confirm.DELETE },
            )
        }
    }
}

private enum class Confirm(val title: String, val text: String, val action: String) {
    NEW_RECOVERY_CODE("Make a new recovery code?", "The old one stops working.", "Make a new one"),
    SIGN_OUT("Sign out of this device?", "Your progress stays in your account; this device starts afresh.", "Sign out"),
    DELETE(
        "Delete your account?",
        "Every check-in, badge and added place is gone for good, on all your devices.",
        "Delete",
    ),
}

@Composable
private fun Heading(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.titleSmall,
        color = LocalAppColors.current.text,
        modifier = Modifier.padding(top = 8.dp),
    )
}

@Composable
private fun ShownCode(code: String, onCopy: () -> Unit) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        SelectionContainer(Modifier.weight(1f, fill = false)) {
            Text(
                code,
                fontFamily = FontFamily.Monospace,
                fontWeight = FontWeight.Bold,
                fontSize = 18.sp,
                letterSpacing = 1.sp,
                color = LocalAppColors.current.text,
            )
        }
        TextButton(onClick = onCopy) { Text("Copy") }
    }
}

private fun ago(at: Long, now: Long): String {
    val days = (now - at) / 86_400
    return when {
        days < 1 -> "today"
        days == 1L -> "yesterday"
        days < 60 -> "$days days ago"
        else -> java.text.DateFormat.getDateInstance().format(java.util.Date(at * 1000))
    }
}
