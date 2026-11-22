package app.mangiamundo.ui

import android.content.Context
import android.content.Intent
import androidx.core.content.FileProvider
import app.mangiamundo.core.ShareLink
import java.io.File

/** Opens the share sheet with a link the server made, its card attached when it loaded. */
fun shareLink(context: Context, link: ShareLink, card: ByteArray?) {
    val send = Intent(Intent.ACTION_SEND).putExtra(Intent.EXTRA_TEXT, "${link.text} ${link.url}")
    if (card != null) {
        // A new file each time, so an app given an earlier card can't read this one. Cards
        // older than an hour go; a share still being written can keep reading its own.
        val dir = File(context.cacheDir, "share").apply { mkdirs() }
        val hourAgo = System.currentTimeMillis() - 3_600_000
        dir.listFiles()?.filter { it.lastModified() < hourAgo }?.forEach { it.delete() }
        val file = File(dir, "mangiamundo-${System.currentTimeMillis()}.png")
        file.writeBytes(card)
        send.setType("image/png")
            .putExtra(Intent.EXTRA_STREAM, FileProvider.getUriForFile(context, "${context.packageName}.share", file))
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
    } else {
        send.setType("text/plain")
    }
    // Started from the view model, outside an activity.
    context.startActivity(Intent.createChooser(send, null).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
}
