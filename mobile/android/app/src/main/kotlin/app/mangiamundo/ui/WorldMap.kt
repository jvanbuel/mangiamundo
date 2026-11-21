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
        animationSpec = infiniteRepeatable(tween(2800, easing = LinearEasing), RepeatMode.Restart),
        label = "phase",
    )

    // One path per fill colour plus one for all borders, reused across frames.
    val fills = remember { List(4) { Path().apply { fillType = PathFillType.EvenOdd } } }
    val star = remember { Path() }
    val facets = remember { Path() }
    val globe = remember { Path() }
    val sparkleAt = remember { ArrayList<Offset>() }
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

        // The flat map repeats sideways, so draw the copies that are in view.
        val copies = if (flat) doubleArrayOf(-360.0, 0.0, 360.0) else doubleArrayOf(0.0)
        for (country in world.countries) {
            val fill = when (statuses[country.code]) {
                null -> fills[0]
                Status.AVAILABLE -> fills[1]
                Status.DISCOVERED -> fills[2]
                Status.COMPLETED -> fills[3]
            }
            // A completed country's sparkles go in the middle of its largest visible ring.
            var largestCount = 0
            var largestX = 0f
            var largestY = 0f
            for (shift in copies) {
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
                    for (i in coordinates.indices step 2) {
                        if (projector.project(coordinates[i] + shift, coordinates[i + 1], point)) {
                            visible = true
                            seen++
                            sumX += point[0]
                            sumY += point[1]
                        }
                        if (i == 0) ring.moveTo(point[0], point[1]) else ring.lineTo(point[0], point[1])
                    }
                    if (!visible) continue // Entirely on the far side of the globe.
                    if (fill === fills[3] && seen > largestCount) {
                        largestCount = seen
                        largestX = sumX / seen
                        largestY = sumY / seen
                    }
                    ring.close()
                    fill.addPath(ring)
                    borders.addPath(ring)
                    if (country.code == selected) outline.addPath(ring)
                }
            }
            if (largestCount > 0) sparkleAt.add(Offset(largestX, largestY))
        }

        drawPath(fills[0], colors.land)
        drawPath(fills[1], colors.available)
        drawPath(fills[2], colors.discovered)
        drawPath(fills[3], colors.completed)

        // Facets cut into each diamond: a lattice of crossing lines.
        if (!fills[3].isEmpty) {
            val box = fills[3].getBounds()
            val step = 7.dp.toPx()
            facets.rewind()
            var x = box.left - box.height
            while (x < box.right) {
                facets.moveTo(x, box.top)
                facets.lineTo(x + box.height, box.bottom)
                facets.moveTo(x + box.height, box.top)
                facets.lineTo(x, box.bottom)
                x += step
            }
            clipPath(fills[3]) { drawPath(facets, colors.completedFacet, style = Stroke(0.8.dp.toPx())) }
        }

        // Thin borders everywhere; each state's countries get an outline in its own colour.
        // On the globe, rings that run over the horizon are flattened onto its rim, so the
        // outlines are clipped to just inside it rather than tracing the rim.
        val edges: DrawScope.() -> Unit = {
            drawPath(borders, colors.mapBorder, style = Stroke(0.6.dp.toPx()))
            drawPath(fills[1], colors.availableEdge, style = Stroke(1.dp.toPx()))
            drawPath(fills[2], colors.discoveredEdge, style = Stroke(1.dp.toPx()))
            drawPath(fills[3], colors.completedEdge, style = Stroke(1.5.dp.toPx()))
            drawPath(outline, colors.text, style = Stroke(2.dp.toPx()))
        }
        if (flat) {
            edges()
        } else {
            globe.rewind()
            globe.addOval(Rect(center, projector.radius.toFloat() - 1.5.dp.toPx()))
            clipPath(globe, block = edges)
        }

        // Sparkles: the diamond's edge colour with a white heart, each fading on its own beat.
        for (at in sparkleAt) {
            SPARKLES.forEachIndexed { i, (dx, dy, tip) ->
                val c = Offset(at.x + dx.dp.toPx(), at.y + dy.dp.toPx())
                val alpha = 0.4f + 0.6f * max(0f, sin(twinkle + i * 2.1f))
                drawStar(star, c, tip.dp.toPx(), colors.completedEdge, alpha)
                drawStar(star, c, tip.dp.toPx() * 0.46f, Color.White, alpha)
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

/** Pixel offsets and tip sizes (dp) of a diamond's three sparkles, so small countries get them too. */
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
