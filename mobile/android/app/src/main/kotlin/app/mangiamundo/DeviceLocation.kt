package app.mangiamundo

import android.Manifest
import android.annotation.SuppressLint
import android.content.Context
import android.content.pm.PackageManager
import android.location.Location
import android.location.LocationManager
import android.os.Build
import android.os.CancellationSignal
import androidx.core.content.ContextCompat
import androidx.core.location.LocationManagerCompat
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withTimeoutOrNull
import kotlin.coroutines.resume

object DeviceLocation {
    val PERMISSIONS = arrayOf(
        Manifest.permission.ACCESS_FINE_LOCATION,
        Manifest.permission.ACCESS_COARSE_LOCATION,
    )

    fun hasPermission(context: Context) = PERMISSIONS.any {
        ContextCompat.checkSelfPermission(context, it) == PackageManager.PERMISSION_GRANTED
    }

    /** A fresh position, falling back to the last known one. Null when none can be had. */
    @SuppressLint("MissingPermission") // Callers check hasPermission first.
    suspend fun current(context: Context): Location? {
        if (!hasPermission(context)) return null
        val manager = context.getSystemService(Context.LOCATION_SERVICE) as LocationManager
        val candidates = buildList {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) add(LocationManager.FUSED_PROVIDER)
            add(LocationManager.NETWORK_PROVIDER)
            add(LocationManager.GPS_PROVIDER)
        }
        val provider = candidates.firstOrNull { runCatching { manager.isProviderEnabled(it) }.getOrDefault(false) }
            ?: return null

        val fresh = withTimeoutOrNull(10_000) {
            suspendCancellableCoroutine<Location?> { continuation ->
                val signal = CancellationSignal()
                continuation.invokeOnCancellation { signal.cancel() }
                LocationManagerCompat.getCurrentLocation(
                    manager, provider, signal, ContextCompat.getMainExecutor(context),
                ) { location -> if (continuation.isActive) continuation.resume(location) }
            }
        }
        return fresh ?: candidates.firstNotNullOfOrNull {
            runCatching { manager.getLastKnownLocation(it) }.getOrNull()
        }
    }
}
