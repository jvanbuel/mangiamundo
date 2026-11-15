package app.mangiamundo.ui

import androidx.compose.animation.core.animate
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
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import app.mangiamundo.core.Camera
import app.mangiamundo.core.ProjectionKind
import app.mangiamundo.core.Projector
import app.mangiamundo.core.Status
import app.mangiamundo.core.World
import app.mangiamundo.core.WorldCountry
import kotlin.math.cos

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

    // One path per fill colour plus one for all borders, reused across frames.
    val fills = remember { List(4) { Path().apply { fillType = PathFillType.EvenOdd } } }
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

        // The flat map repeats sideways, so draw the copies that are in view.
        val copies = if (flat) doubleArrayOf(-360.0, 0.0, 360.0) else doubleArrayOf(0.0)
        for (country in world.countries) {
            val fill = when (statuses[country.code]) {
                null -> fills[0]
                Status.AVAILABLE -> fills[1]
                Status.DISCOVERED -> fills[2]
                Status.COMPLETED -> fills[3]
            }
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
                    for (i in coordinates.indices step 2) {
                        if (projector.project(coordinates[i] + shift, coordinates[i + 1], point)) visible = true
                        if (i == 0) ring.moveTo(point[0], point[1]) else ring.lineTo(point[0], point[1])
                    }
                    if (!visible) continue // Entirely on the far side of the globe.
                    ring.close()
                    fill.addPath(ring)
                    borders.addPath(ring)
                    if (country.code == selected) outline.addPath(ring)
                }
            }
        }

        drawPath(fills[0], colors.land)
        drawPath(fills[1], colors.available)
        drawPath(fills[2], colors.discovered)
        drawPath(fills[3], colors.completed)
        drawPath(borders, colors.ocean, style = Stroke(0.6.dp.toPx()))
        drawPath(outline, colors.text, style = Stroke(2.dp.toPx()))

        if (here != null && projector.project(here.first, here.second, point)) {
            val at = Offset(point[0], point[1])
            drawCircle(colors.text.copy(alpha = 0.25f), 12.dp.toPx(), at)
            drawCircle(colors.panel, 6.5.dp.toPx(), at)
            drawCircle(colors.text, 4.5.dp.toPx(), at)
        }
    }
}
