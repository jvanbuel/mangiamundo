package app.mangiamundo.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.MutableState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import app.mangiamundo.AppViewModel
import app.mangiamundo.core.Identity

/**
 * The account section's "Sign in": how the account is signed in, or Google and an emailed
 * code. What was typed lives in the section, in `emailState` and `signInCodeState`.
 */
@Composable
internal fun SignInSection(
    vm: AppViewModel,
    identities: List<Identity>,
    emailState: MutableState<String>,
    signInCodeState: MutableState<String>,
    onSignOut: () -> Unit,
) {
    val colors = LocalAppColors.current
    val small = MaterialTheme.typography.bodySmall
    val context = LocalContext.current
    var email by emailState
    var signInCode by signInCodeState
    Heading("Sign in")
    if (identities.isNotEmpty()) {
        val names = mapOf("apple" to "Apple", "google" to "Google", "email" to "email")
        val list = identities.joinToString { i -> (names[i.provider] ?: i.provider) + (i.email?.let { " ($it)" } ?: "") }
        Text("Signed in with $list.", color = colors.muted, style = small)
        ChunkyButton("Sign out of this device", onClick = onSignOut, enabled = !vm.accountBusy)
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
}
