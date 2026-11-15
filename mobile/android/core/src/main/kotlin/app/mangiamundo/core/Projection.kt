package app.mangiamundo.core

import kotlin.math.PI
import kotlin.math.asin
import kotlin.math.atan
import kotlin.math.atan2
import kotlin.math.cos
import kotlin.math.exp
import kotlin.math.hypot
import kotlin.math.ln
import kotlin.math.min
import kotlin.math.sin
import kotlin.math.tan

enum class ProjectionKind { GLOBE, FLAT }

/** What the map looks at: the centre of the view and a zoom factor (1 = whole world). */
data class Camera(val lng: Double = 10.0, val lat: Double = 30.0, val zoom: Double = 1.0) {
    fun normalized() = copy(
        lng = ((lng + 180.0) % 360.0 + 360.0) % 360.0 - 180.0,
        lat = lat.coerceIn(-MAX_LAT, MAX_LAT),
        zoom = zoom.coerceIn(MIN_ZOOM, MAX_ZOOM),
    )

    companion object {
        const val MAX_LAT = 80.0
        const val MIN_ZOOM = 0.8
        const val MAX_ZOOM = 8.0
    }
}

/**
 * Maps between geographic coordinates (degrees) and view pixels. The globe is an
 * orthographic projection, the flat map a Web Mercator one.
 */
class Projector(val kind: ProjectionKind, camera: Camera, val width: Double, val height: Double) {
    private val lng0 = Math.toRadians(camera.lng)
    private val lat0 = Math.toRadians(camera.lat)
    private val sin0 = sin(lat0)
    private val cos0 = cos(lat0)
    private val cx = width / 2
    private val cy = height / 2

    /** Globe radius in pixels. */
    val radius = min(width, height) / 2 * 0.92 * camera.zoom

    /** Pixels per radian on the flat map; the world is `2π · scale` wide. */
    val scale = width / (2 * PI) * camera.zoom
    val worldWidth get() = 2 * PI * scale

    private val mercator0 = mercatorY(lat0)

    /** Degrees of longitude (and, on the globe, latitude) that one pixel of dragging covers. */
    val degreesPerPixel: Double
        get() = Math.toDegrees(1 / if (kind == ProjectionKind.GLOBE) radius else scale)

    /**
     * Writes the pixel position of a coordinate into `out[0..1]`. Returns false when the
     * point is on the far side of the globe; it is then placed on the edge of the disc,
     * which keeps partly hidden outlines closed.
     */
    fun project(lng: Double, lat: Double, out: FloatArray): Boolean {
        val l = Math.toRadians(lng) - lng0
        val p = Math.toRadians(lat)
        if (kind == ProjectionKind.FLAT) {
            out[0] = (cx + l * scale).toFloat()
            out[1] = (cy - (mercatorY(p) - mercator0) * scale).toFloat()
            return true
        }
        val cosP = cos(p)
        val x = cosP * sin(l)
        val y = cos0 * sin(p) - sin0 * cosP * cos(l)
        val facing = sin0 * sin(p) + cos0 * cosP * cos(l) >= 0
        val stretch = if (facing) 1.0 else 1 / hypot(x, y).coerceAtLeast(1e-9)
        out[0] = (cx + x * stretch * radius).toFloat()
        out[1] = (cy - y * stretch * radius).toFloat()
        return facing
    }

    /** The coordinate under a pixel as (lng, lat), or null outside the globe. */
    fun unproject(x: Double, y: Double): Pair<Double, Double>? {
        if (kind == ProjectionKind.FLAT) {
            val lng = Math.toDegrees(lng0 + (x - cx) / scale)
            val lat = Math.toDegrees(2 * atan(exp(mercator0 - (y - cy) / scale)) - PI / 2)
            return wrap(lng) to lat
        }
        val dx = (x - cx) / radius
        val dy = -(y - cy) / radius
        val rho = hypot(dx, dy)
        if (rho > 1) return null
        if (rho < 1e-12) return wrap(Math.toDegrees(lng0)) to Math.toDegrees(lat0)
        val c = asin(rho)
        val lat = asin(cos(c) * sin0 + dy * sin(c) * cos0 / rho)
        val lng = lng0 + atan2(dx * sin(c), rho * cos0 * cos(c) - dy * sin0 * sin(c))
        return wrap(Math.toDegrees(lng)) to Math.toDegrees(lat)
    }

    private fun mercatorY(lat: Double) = ln(tan(PI / 4 + lat.coerceIn(-MAX_MERCATOR, MAX_MERCATOR) / 2))

    private fun wrap(lng: Double) = ((lng + 180.0) % 360.0 + 360.0) % 360.0 - 180.0

    private companion object {
        val MAX_MERCATOR = Math.toRadians(85.0)
    }
}
