package app.mangiamundo.ui

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animate
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.detectTransformGestures
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipPath
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import app.mangiamundo.core.Camera
import app.mangiamundo.core.ProjectionKind
import app.mangiamundo.core.Projector
import app.mangiamundo.core.Status
import app.mangiamundo.core.World
import app.mangiamundo.core.WorldCountry
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.max
import kotlin.math.min
import kotlin.math.sin

/**
 * The world as a globe or a flat map, drawn directly from the country outlines.
 * Drag to move, pinch to zoom, tap a country to pick it.
 */
@Composable
fun WorldMap(
    world: World,
    statuses: Map<String, Status>,
    selected: String?,
    here: Pair<Double, Double>?,
    kind: ProjectionKind,
    camera: Camera,
    onCamera: (Camera) -> Unit,
    onTap: (WorldCountry) -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = LocalAppColors.current
    val currentCamera by rememberUpdatedState(camera)
    val currentOnCamera by rememberUpdatedState(onCamera)
    val currentOnTap by rememberUpdatedState(onTap)

    // Glide to the user's location whenever it changes.
    LaunchedEffect(here) {
        val (lng, lat) = here ?: return@LaunchedEffect
        val start = currentCamera
        // Take the short way around the globe.
        val delta = ((lng - start.lng + 540) % 360) - 180
        animate(0f, 1f, animationSpec = tween(900)) { t, _ ->
            currentOnCamera(start.copy(lng = start.lng + delta * t, lat = start.lat + (lat - start.lat) * t).normalized())
        }
    }

    fun projector(size: IntSize) =
        Projector(kind, currentCamera, size.width.toDouble(), size.height.toDouble())

    // Diamonds twinkle: one phase drives all sparkles, each on its own beat. With
    // animations turned off in the system settings, Compose holds it still.
    val twinkle by rememberInfiniteTransition(label = "twinkle").animateFloat(
        initialValue = 0f,
        targetValue = (2 * PI).toFloat(),
        animationSpec = infiniteRepeatable(tween(2400, easing = LinearEasing), RepeatMode.Restart),
        label = "phase",
    )
    // Now and then a rainbow glint sweeps across each diamond: 0 to 1 every 4 seconds.
    val glint by rememberInfiniteTransition(label = "glint").animateFloat(
        initialValue = 0f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(tween(4000, easing = LinearEasing), RepeatMode.Restart),
        label = "sweep",
    )

    // One path per fill colour plus one for all borders, reused across frames.
    val fills = remember { List(5) { Path().apply { fillType = PathFillType.EvenOdd } } }
    val star = remember { Path() }
    val globe = remember { Path() }
    // Where each completed country's sparkles go, and how much they shrink to fit it.
    val sparkleAt = remember { ArrayList<Pair<Offset, Float>>() }
    // Bordering diamonds shine as one surface: one path per run of completed neighbours
    // (and per copy of the flat map).
    val gems = remember { HashMap<String, Path>() }
    val regions = remember(world, statuses) { world.regions(statuses.filterValues { it == Status.COMPLETED }.keys) }
    val borders = remember { Path() }
    val outline = remember { Path() }
    val ring = remember { Path() }
    val point = remember { FloatArray(2) }

    Canvas(
        modifier
            .clipToBounds()
            .pointerInput(kind) {
                detectTransformGestures { _, pan, zoom, _ ->
                    val now = currentCamera
                    val perPixel = projector(size).degreesPerPixel
                    // On the flat map a pixel covers fewer degrees of latitude towards the poles.
                    val latFactor = if (kind == ProjectionKind.FLAT) cos(Math.toRadians(now.lat)) else 1.0
                    currentOnCamera(
                        Camera(
                            lng = now.lng - pan.x * perPixel,
                            lat = now.lat + pan.y * perPixel * latFactor,
                            zoom = now.zoom * zoom,
                        ).normalized(),
                    )
                }
            }
            .pointerInput(kind, world) {
                detectTapGestures { at ->
                    val (lng, lat) = projector(size).unproject(at.x.toDouble(), at.y.toDouble())
                        ?: return@detectTapGestures
                    world.countryAt(lng, lat)?.let { currentOnTap(it) }
                }
            },
    ) {
        val projector = Projector(kind, camera, size.width.toDouble(), size.height.toDouble())
        val flat = kind == ProjectionKind.FLAT

        if (flat) {
            drawRect(colors.ocean)
        } else {
            drawCircle(colors.ocean, projector.radius.toFloat(), center)
        }

        fills.forEach { it.rewind() }
        borders.rewind()
        outline.rewind()
        sparkleAt.clear()
        gems.clear()

        // The flat map repeats sideways, so draw the copies that are in view.
        val copies = if (flat) doubleArrayOf(-360.0, 0.0, 360.0) else doubleArrayOf(0.0)
        for (country in world.countries) {
            val fill = when (statuses[country.code]) {
                null -> fills[0]
                Status.AVAILABLE -> fills[1]
                Status.DISCOVERED -> fills[2]
                Status.COMPLETED -> fills[3]
                Status.PLATINUM -> fills[4]
            }
            // A completed country's sparkles go in the middle of its largest visible ring.
            var largestCount = 0
            var largestX = 0f
            var largestY = 0f
            var largestSide = 0f
            val region = regions[country.code]
            for (shift in copies) {
                val gem = region?.let { gems.getOrPut("$it $shift") { Path().apply { fillType = PathFillType.EvenOdd } } }
                if (flat) {
                    projector.project(country.maxLng + shift, 0.0, point)
                    if (point[0] < 0) continue
                    projector.project(country.minLng + shift, 0.0, point)
                    if (point[0] > size.width) continue
                }
                for (coordinates in country.rings) {
                    ring.rewind()
                    var visible = false
                    var seen = 0
                    var sumX = 0f
                    var sumY = 0f
                    var minX = Float.MAX_VALUE
                    var minY = Float.MAX_VALUE
                    var maxX = -Float.MAX_VALUE
                    var maxY = -Float.MAX_VALUE
                    for (i in coordinates.indices step 2) {
                        if (projector.project(coordinates[i] + shift, coordinates[i + 1], point)) {
                            visible = true
                            seen++
                            sumX += point[0]
                            sumY += point[1]
                            minX = min(minX, point[0]); maxX = max(maxX, point[0])
                            minY = min(minY, point[1]); maxY = max(maxY, point[1])
                        }
                        if (i == 0) ring.moveTo(point[0], point[1]) else ring.lineTo(point[0], point[1])
                    }
                    if (!visible) continue // Entirely on the far side of the globe.
                    if (fill === fills[3] && seen > largestCount) {
                        largestCount = seen
                        largestX = sumX / seen
                        largestY = sumY / seen
                        largestSide = min(maxX - minX, maxY - minY)
                    }
                    ring.close()
                    fill.addPath(ring)
                    gem?.addPath(ring)
                    borders.addPath(ring)
                    if (country.code == selected) outline.addPath(ring)
                }
            }
            if (largestCount > 0) {
                // The cluster is about 42dp across at full size; it shrinks to fit smaller countries.
                val fit = largestSide / SPARKLE_SPAN.dp.toPx()
                sparkleAt.add(Offset(largestX, largestY) to fit.coerceIn(0.2f, 1f))
            }
        }

        drawPath(fills[0], colors.land)
        drawPath(fills[1], colors.available)
        drawPath(fills[2], colors.discovered)
        drawPath(fills[3], colors.completed)
        drawPath(fills[4], colors.platinum)

        // Each diamond shines: lit from the top left, deeper towards the bottom right, and now
        // and then a glint sweeps across it, a white heart with rainbow fringes, like light
        // split by a cut stone. With animations off the sweep holds before it starts.
        for (gem in gems.values) {
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

        // Thin borders everywhere; each state's countries get an outline in its own colour.
        // On the globe, rings that run over the horizon are flattened onto its rim, so the
        // outlines are clipped to just inside it rather than tracing the rim.
        val edges: DrawScope.() -> Unit = {
            drawPath(borders, colors.mapBorder, style = Stroke(0.6.dp.toPx()))
            drawPath(fills[1], colors.availableEdge, style = Stroke(1.dp.toPx()))
            drawPath(fills[2], colors.discoveredEdge, style = Stroke(1.dp.toPx()))
            drawPath(fills[3], colors.completedEdge, style = Stroke(1.5.dp.toPx()))
            drawPath(fills[4], colors.platinumEdge, style = Stroke(1.dp.toPx()))
            drawPath(outline, colors.text, style = Stroke(2.dp.toPx()))
        }
        if (flat) {
            edges()
        } else {
            globe.rewind()
            globe.addOval(Rect(center, projector.radius.toFloat() - 1.5.dp.toPx()))
            clipPath(globe, block = edges)
        }

        // Sparkles: the diamond's edge colour with a white heart. Each flashes on its own
        // beat, growing and brightening quickly and then shrinking back.
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

        if (here != null && projector.project(here.first, here.second, point)) {
            val at = Offset(point[0], point[1])
            drawCircle(colors.text.copy(alpha = 0.25f), 12.dp.toPx(), at)
            drawCircle(colors.panel, 6.5.dp.toPx(), at)
            drawCircle(colors.text, 4.5.dp.toPx(), at)
        }
    }
}

/** Width (dp) of the sparkle cluster at full size. */
private const val SPARKLE_SPAN = 42f

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
