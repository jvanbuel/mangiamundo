package app.mangiamundo.ui

import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.RectF
import androidx.compose.ui.graphics.toArgb
import androidx.core.content.FileProvider
import androidx.core.content.res.ResourcesCompat
import app.mangiamundo.R
import java.io.File
import kotlin.math.max

/** Puts a caption under the map, 1200 × 630 like link previews, and opens the share sheet with it. */
fun shareMap(context: Context, map: Bitmap, headline: String, caption: String, message: String, colors: AppColors) {
    val w = 1200
    val h = 630
    val strip = 150
    val card = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888)
    val canvas = Canvas(card)
    canvas.drawColor(colors.bg.toArgb())
    // Cover the area above the caption, cropping the map's centre.
    val scale = max(w / map.width.toFloat(), (h - strip) / map.height.toFloat())
    val dw = map.width * scale
    val dh = map.height * scale
    canvas.save()
    canvas.clipRect(0, 0, w, h - strip)
    canvas.drawBitmap(map, null, RectF((w - dw) / 2, (h - strip - dh) / 2, (w + dw) / 2, (h - strip + dh) / 2), Paint(Paint.FILTER_BITMAP_FLAG))
    canvas.restore()
    canvas.drawRect(0f, (h - strip).toFloat(), w.toFloat(), h.toFloat(), Paint().apply { color = colors.panel.toArgb() })
    fun text(value: String, y: Int, size: Float, font: Int, color: Int) = canvas.drawText(
        value, 48f, (h - strip + y).toFloat(),
        Paint(Paint.ANTI_ALIAS_FLAG).apply {
            textSize = size
            typeface = ResourcesCompat.getFont(context, font)
            this.color = color
        },
    )
    text(headline, 70, 52f, R.font.nunito_extrabold, colors.text.toArgb())
    text(caption, 116, 30f, R.font.nunito_semibold, colors.muted.toArgb())

    val file = File(context.cacheDir, "share/mangiamundo.png").apply { parentFile?.mkdirs() }
    file.outputStream().use { card.compress(Bitmap.CompressFormat.PNG, 100, it) }
    val uri = FileProvider.getUriForFile(context, "${context.packageName}.share", file)
    val send = Intent(Intent.ACTION_SEND)
        .setType("image/png")
        .putExtra(Intent.EXTRA_STREAM, uri)
        .putExtra(Intent.EXTRA_TEXT, message)
        .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
    context.startActivity(Intent.createChooser(send, null))
}
