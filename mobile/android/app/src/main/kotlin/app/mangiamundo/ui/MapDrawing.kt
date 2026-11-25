package app.mangiamundo.ui

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipPath
import androidx.compose.ui.unit.dp
import app.mangiamundo.core.Projector
import kotlin.math.PI
import kotlin.math.max
import kotlin.math.sin

// The layers [WorldMap] draws over the country fills.

/**
 * Thin borders everywhere; each state's countries get an outline in its own colour.
 * On the globe, rings that run over the horizon are flattened onto its rim, so the
 * outlines are clipped to just inside it rather than tracing the rim. `globe` is the
 * path to clip with, reused across frames; null on the flat map.
 */
internal fun DrawScope.drawEdges(fills: List<Path>, borders: Path, outline: Path, globe: Path?, projector: Projector, colors: AppColors) {
    val edges: DrawScope.() -> Unit = {
        drawPath(borders, colors.mapBorder, style = Stroke(0.6.dp.toPx()))
        drawPath(fills[1], colors.availableEdge, style = Stroke(1.dp.toPx()))
        drawPath(fills[2], colors.discoveredEdge, style = Stroke(1.dp.toPx()))
        drawPath(fills[3], colors.completedEdge, style = Stroke(1.5.dp.toPx()))
        drawPath(fills[4], colors.platinumEdge, style = Stroke(1.dp.toPx()))
        drawPath(outline, colors.text, style = Stroke(2.dp.toPx()))
    }
    if (globe == null) {
        edges()
    } else {
        globe.rewind()
        globe.addOval(Rect(center, projector.radius.toFloat() - 1.5.dp.toPx()))
        clipPath(globe, block = edges)
    }
}

/** The user's position: a dot with a ring around it. */
internal fun DrawScope.drawHere(at: Offset, colors: AppColors) {
    drawCircle(colors.text.copy(alpha = 0.25f), 12.dp.toPx(), at)
    drawCircle(colors.panel, 6.5.dp.toPx(), at)
    drawCircle(colors.text, 4.5.dp.toPx(), at)
}

/**
 * Each diamond shines: lit from the top left, deeper towards the bottom right, and now
 * and then a glint sweeps across it, a white heart with rainbow fringes, like light
 * split by a cut stone. With animations off the sweep holds before it starts.
 */
internal fun DrawScope.drawGems(gems: Collection<Path>, glint: Float, colors: AppColors) {
    for (gem in gems) {
        if (gem.isEmpty) continue
        val box = gem.getBounds()
        clipPath(gem) {
            drawRect(
                Brush.linearGradient(
                    0f to Color.White, 0.45f to colors.completed, 1f to colors.completedFacet,
                    start = box.topLeft, end = box.bottomRight,
                ),
                box.topLeft, box.size,
            )
            if (glint > 0.7f) {
                val t = (glint - 0.7f) / 0.3f
                val c = Offset(box.left + t * box.width, box.top + t * box.height)
                val d = max(box.width, box.height) * 0.18f
                drawRect(
                    Brush.linearGradient(
                        0f to Color.Red.copy(alpha = 0f),
                        0.15f to Color.Red.copy(alpha = 0.35f),
                        0.27f to Color(0xFFFF9500).copy(alpha = 0.45f),
                        0.38f to Color.Yellow.copy(alpha = 0.55f),
                        0.5f to Color.White.copy(alpha = 0.9f),
                        0.62f to Color.Cyan.copy(alpha = 0.55f),
                        0.73f to Color.Blue.copy(alpha = 0.45f),
                        0.85f to Color(0xFFAF52DE).copy(alpha = 0.35f),
                        1f to Color(0xFFAF52DE).copy(alpha = 0f),
                        start = Offset(c.x - d, c.y - d), end = Offset(c.x + d, c.y + d),
                    ),
                    box.topLeft, box.size,
                )
            }
        }
    }
}

/**
 * Sparkles: the diamond's edge colour with a white heart. Each flashes on its own
 * beat, growing and brightening quickly and then shrinking back.
 */
internal fun DrawScope.drawSparkles(sparkleAt: List<Pair<Offset, Float>>, twinkle: Float, star: Path, colors: AppColors) {
    for ((at, scale) in sparkleAt) {
        SPARKLES.forEachIndexed { i, (dx, dy, tip) ->
            val c = Offset(at.x + dx.dp.toPx() * scale, at.y + dy.dp.toPx() * scale)
            // Offset so the phase's starting point, all that shows when animations are off, has
            // the main sparkle at full size.
            val wave = max(0f, sin(twinkle + i * 2.1f + (PI / 2).toFloat()))
            val flash = wave * wave
            val alpha = 0.25f + 0.75f * flash
            val size = tip.dp.toPx() * scale * (0.55f + 0.45f * flash)
            drawStar(star, c, size, colors.completedEdge, alpha)
            drawStar(star, c, size * 0.46f, Color.White, alpha)
        }
    }
}

/** Width (dp) of the sparkle cluster at full size. */
internal const val SPARKLE_SPAN = 42f

/** Offsets and tip sizes (dp) of a diamond's three sparkles at full size. */
private val SPARKLES = listOf(Triple(0f, 0f, 11f), Triple(14f, -12f, 7f), Triple(-16f, 13f, 5.5f))

/** A four-pointed star: tips at [tip] from the centre, sides curving in towards it. */
private fun DrawScope.drawStar(path: Path, c: Offset, tip: Float, color: Color, alpha: Float) {
    val dirs = listOf(0f to -1f, 1f to 0f, 0f to 1f, -1f to 0f)
    val waist = tip * 0.25f
    path.rewind()
    path.moveTo(c.x, c.y - tip)
    dirs.forEachIndexed { i, (dx, dy) ->
        val (nx, ny) = dirs[(i + 1) % 4]
        path.quadraticTo(c.x + (dx + nx) * waist, c.y + (dy + ny) * waist, c.x + nx * tip, c.y + ny * tip)
    }
    path.close()
    drawPath(path, color, alpha = alpha)
}
