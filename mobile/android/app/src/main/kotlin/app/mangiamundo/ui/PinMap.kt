package app.mangiamundo.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Place
import androidx.compose.material3.Icon
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import app.mangiamundo.core.Place
import androidx.compose.foundation.layout.offset
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.viewinterop.AndroidView
import org.maplibre.android.MapLibre
import org.maplibre.android.camera.CameraPosition
import org.maplibre.android.geometry.LatLng as MapLatLng
import org.maplibre.android.maps.MapView

/** A street map with a pin fixed in its middle; `onMove` gets the point under the pin. */
@Composable
internal fun PinMap(lat: Double, lng: Double, onMove: (Double, Double) -> Unit, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    val moved by rememberUpdatedState(onMove)
    val mapView = remember {
        MapLibre.getInstance(context)
        MapView(context).apply {
            onCreate(null)
            // Drags move the map, not the scrolling sheet around it.
            setOnTouchListener { view, _ ->
                view.parent?.requestDisallowInterceptTouchEvent(true)
                false
            }
            getMapAsync { map ->
                // Free street tiles from OpenStreetMap data, no key needed.
                map.setStyle("https://tiles.openfreemap.org/styles/liberty")
                map.cameraPosition = CameraPosition.Builder().target(MapLatLng(lat, lng)).zoom(17.0).build()
                map.uiSettings.isRotateGesturesEnabled = false
                map.uiSettings.isTiltGesturesEnabled = false
                map.addOnCameraMoveListener { map.cameraPosition.target?.let { moved(it.latitude, it.longitude) } }
            }
        }
    }
    // MapView follows the screen's lifecycle; adding the observer replays the events so far.
    DisposableEffect(lifecycle) {
        val observer = LifecycleEventObserver { _, event ->
            when (event) {
                Lifecycle.Event.ON_START -> mapView.onStart()
                Lifecycle.Event.ON_RESUME -> mapView.onResume()
                Lifecycle.Event.ON_PAUSE -> mapView.onPause()
                Lifecycle.Event.ON_STOP -> mapView.onStop()
                else -> Unit
            }
        }
        lifecycle.addObserver(observer)
        onDispose {
            lifecycle.removeObserver(observer)
            mapView.onPause()
            mapView.onStop()
            mapView.onDestroy()
        }
    }
    Box(modifier) {
        AndroidView({ mapView }, Modifier.fillMaxWidth().fillMaxHeight())
        // The pin's tip marks the map's centre.
        Icon(
            Icons.Rounded.Place,
            contentDescription = null,
            tint = LocalAppColors.current.accent,
            modifier = Modifier.align(Alignment.Center).offset(y = (-17).dp).size(34.dp),
        )
    }
}
