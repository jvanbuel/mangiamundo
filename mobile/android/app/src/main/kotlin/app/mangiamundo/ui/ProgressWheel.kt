package app.mangiamundo.ui

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import app.mangiamundo.core.AreaProgress

/** A city's restaurants loading: the percentage in a ring, with what the server is doing below. */
@Composable
fun LoadingWheel(progress: AreaProgress, modifier: Modifier = Modifier) {
    val colors = LocalAppColors.current
    val animated by animateFloatAsState(progress.percent / 100f, label = "area progress")
    Column(
        modifier
            .clip(RoundedCornerShape(14.dp))
            .background(colors.panel)
            .padding(20.dp)
            .semantics(mergeDescendants = true) { contentDescription = "${progress.text}, ${progress.percent} percent" },
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Box(contentAlignment = Alignment.Center) {
            CircularProgressIndicator(
                progress = { animated },
                modifier = Modifier.size(72.dp),
                color = colors.accent,
                strokeWidth = 6.dp,
                trackColor = colors.line,
            )
            Text("${progress.percent}%", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.ExtraBold, color = colors.text)
        }
        Text(progress.text, style = MaterialTheme.typography.bodyMedium, color = colors.muted, modifier = Modifier.padding(top = 10.dp))
    }
}
