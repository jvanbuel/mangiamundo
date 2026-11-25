package app.mangiamundo.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Casino
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.mangiamundo.AppViewModel
import app.mangiamundo.core.Discover
import app.mangiamundo.core.ProjectionKind
import app.mangiamundo.core.Status

/** The top of the home screen: the map, with the Spin button, the projection switch and the legend. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun MapPane(vm: AppViewModel, discover: Discover?, statuses: Map<String, Status>, modifier: Modifier) {
    val colors = LocalAppColors.current
    Box(modifier.background(colors.bg)) {
        vm.world?.let { world ->
            WorldMap(
                world = world,
                statuses = statuses,
                selected = vm.selected,
                here = vm.location?.let { it.lng to it.lat },
                kind = vm.projection,
                camera = vm.camera,
                onCamera = { vm.camera = it },
                onTap = { vm.tapCountry(it.code, it.name) },
                modifier = Modifier.fillMaxSize(),
            )
        }
        if (discover != null) {
            ChunkyButton(
                if (vm.spinning) "Spinning…" else "Spin",
                vm::spin,
                Modifier
                    .align(Alignment.TopStart)
                    .windowInsetsPadding(WindowInsets.statusBars)
                    .padding(8.dp),
                enabled = !vm.spinning,
                icon = Icons.Rounded.Casino,
            )
        }
        SingleChoiceSegmentedButtonRow(
            Modifier
                .align(Alignment.TopEnd)
                .windowInsetsPadding(WindowInsets.statusBars)
                .padding(8.dp),
        ) {
            val kinds = listOf(ProjectionKind.GLOBE to "Globe", ProjectionKind.FLAT to "Flat")
            kinds.forEachIndexed { index, (kind, label) ->
                SegmentedButton(
                    selected = vm.projection == kind,
                    onClick = { vm.changeProjection(kind) },
                    shape = SegmentedButtonDefaults.itemShape(index, kinds.size),
                    icon = {},
                ) { Text(label) }
            }
        }
        Legend(Modifier.align(Alignment.BottomStart).padding(8.dp))
    }
}
