package app.mangiamundo.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.mangiamundo.AppViewModel
import app.mangiamundo.core.Device

// Pieces of the account section in the settings sheet.

/** The devices signed in to the account, each but this one with a Sign out button. */
@Composable
internal fun DeviceList(vm: AppViewModel, devices: List<Device>, now: Long) {
    val colors = LocalAppColors.current
    val small = MaterialTheme.typography.bodySmall
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

@Composable
internal fun Heading(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.titleSmall,
        color = LocalAppColors.current.text,
        modifier = Modifier.padding(top = 8.dp),
    )
}

@Composable
internal fun ShownCode(code: String, onCopy: () -> Unit) {
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

internal fun ago(at: Long, now: Long): String {
    val days = (now - at) / 86_400
    return when {
        days < 1 -> "today"
        days == 1L -> "yesterday"
        days < 60 -> "$days days ago"
        else -> java.text.DateFormat.getDateInstance().format(java.util.Date(at * 1000))
    }
}
