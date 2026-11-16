package app.mangiamundo.ui

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.graphics.BitmapFactory
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.ContentCopy
import androidx.compose.material.icons.rounded.Share
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.FileProvider
import app.mangiamundo.AppViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File

/** Shows the recovery code and lets the user restore another passport onto this phone. */
@Composable
fun BackupSheet(vm: AppViewModel, onDone: () -> Unit) {
    val colors = LocalAppColors.current
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var typed by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    var confirmRotate by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { vm.loadRecoveryCode() }

    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Text("Back up your passport", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Black, color = colors.text)
        Text(
            "Your progress is tied to this phone. Keep this code somewhere safe to get it back on a new " +
                "phone or on the web. Anyone with the code can open your passport.",
            style = MaterialTheme.typography.bodyMedium,
            color = colors.muted,
        )
        Box(
            Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(12.dp))
                .background(colors.panelRaised)
                .padding(14.dp),
        ) {
            Column {
                Text("Recovery code", style = MaterialTheme.typography.bodySmall, fontWeight = FontWeight.Bold, color = colors.muted)
                Text(
                    vm.recoveryCode ?: "Loading…",
                    fontFamily = FontFamily.Monospace,
                    fontWeight = FontWeight.Bold,
                    fontSize = 20.sp,
                    color = colors.text,
                    modifier = Modifier.padding(vertical = 4.dp),
                )
                Row {
                    TextButton(onClick = { vm.recoveryCode?.let { copy(context, it) } }, enabled = vm.recoveryCode != null) {
                        Icon(Icons.Rounded.ContentCopy, contentDescription = null, modifier = Modifier.size(16.dp))
                        Text(" Copy")
                    }
                    TextButton(onClick = { confirmRotate = true }, enabled = vm.recoveryCode != null) { Text("New code") }
                }
            }
        }
        HorizontalDivider(color = colors.line, modifier = Modifier.padding(vertical = 6.dp))
        Text("Restore from another device", style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.ExtraBold, color = colors.text)
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedTextField(
                value = typed,
                onValueChange = { typed = it.uppercase() },
                placeholder = { Text("XXXX-XXXX-XXXX-XXXX", fontFamily = FontFamily.Monospace) },
                singleLine = true,
                textStyle = MaterialTheme.typography.bodyLarge.copy(fontFamily = FontFamily.Monospace),
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Characters, autoCorrectEnabled = false),
                modifier = Modifier.weight(1f),
            )
            ChunkyButton(
                if (busy) "Restoring…" else "Restore",
                onClick = {
                    busy = true
                    scope.launch {
                        if (vm.restore(typed)) onDone()
                        busy = false
                    }
                },
                enabled = !busy && typed.count { it.isLetterOrDigit() } == 16,
            )
        }
        if ((vm.discover?.profile?.visits ?: 0) > 0) {
            Text(
                "The check-ins made on this phone are added to the restored passport.",
                style = MaterialTheme.typography.bodySmall,
                color = colors.muted,
            )
        }
    }

    if (confirmRotate) {
        AlertDialog(
            onDismissRequest = { confirmRotate = false },
            title = { Text("Make a new code?") },
            text = { Text("The old code stops working.") },
            confirmButton = {
                TextButton(onClick = {
                    confirmRotate = false
                    vm.rotateRecoveryCode()
                }) { Text("New code") }
            },
            dismissButton = { TextButton(onClick = { confirmRotate = false }) { Text("Cancel") } },
        )
    }
}

private fun copy(context: Context, text: String) {
    val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
    clipboard.setPrimaryClip(ClipData.newPlainText("Mangiamundo recovery code", text))
}

/** A preview of the server-drawn share card, with a button to the system share sheet. */
@Composable
fun ShareSheet(vm: AppViewModel, dark: Boolean) {
    val colors = LocalAppColors.current
    val context = LocalContext.current
    var png by remember { mutableStateOf<ByteArray?>(null) }
    var failed by remember { mutableStateOf(false) }
    LaunchedEffect(dark) {
        png = vm.shareCard(dark)
        failed = png == null
    }
    val bitmap = remember(png) { png?.let { BitmapFactory.decodeByteArray(it, 0, it.size)?.asImageBitmap() } }
    val tasted = vm.discover?.world?.discovered ?: 0
    val city = vm.location?.name ?: "my city"
    val text = if (tasted > 0) {
        "I've tasted $tasted ${if (tasted == 1) "country" else "countries"} without leaving $city. Eat the world with Mangiamundo!"
    } else {
        "I'm eating my way around the world in $city with Mangiamundo."
    }

    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text("Share your map", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Black, color = colors.text)
        Box(
            Modifier
                .fillMaxWidth()
                .aspectRatio(4f / 5f)
                .clip(RoundedCornerShape(14.dp))
                .background(colors.panelRaised),
            contentAlignment = Alignment.Center,
        ) {
            when {
                bitmap != null -> Image(bitmap, contentDescription = text, contentScale = ContentScale.Fit, modifier = Modifier.fillMaxWidth())
                failed -> Text("Could not draw the card.", color = colors.muted)
                else -> Text("Drawing your card…", color = colors.muted)
            }
        }
        ChunkyButton(
            "Share",
            onClick = { png?.let { share(context, it, text) } },
            modifier = Modifier.fillMaxWidth(),
            enabled = png != null,
            icon = Icons.Rounded.Share,
        )
    }
}

private fun share(context: Context, png: ByteArray, text: String) {
    val dir = File(context.cacheDir, "shared").apply { mkdirs() }
    val file = File(dir, "mangiamundo.png").apply { writeBytes(png) }
    val uri = FileProvider.getUriForFile(context, "${context.packageName}.files", file)
    val send = Intent(Intent.ACTION_SEND).apply {
        type = "image/png"
        putExtra(Intent.EXTRA_STREAM, uri)
        putExtra(Intent.EXTRA_TEXT, text)
        addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
    }
    context.startActivity(Intent.createChooser(send, "Share your map"))
}
