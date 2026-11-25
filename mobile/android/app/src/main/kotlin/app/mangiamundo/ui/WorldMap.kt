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
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathFillType
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

        drawGems(gems.values, glint, colors)

        drawEdges(fills, borders, outline, if (flat) null else globe, projector, colors)

        drawSparkles(sparkleAt, twinkle, star, colors)

        if (here != null && projector.project(here.first, here.second, point)) {
            drawHere(Offset(point[0], point[1]), colors)
        }
    }
}
