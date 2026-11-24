package app.mangiamundo.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.mangiamundo.core.Country
import app.mangiamundo.core.CountryProgress
import app.mangiamundo.core.GroupProgress

@Composable
fun ContinentHeader(continent: GroupProgress) {
    val colors = LocalAppColors.current
    Row(Modifier.fillMaxWidth().padding(top = 16.dp, bottom = 4.dp, start = 8.dp, end = 8.dp)) {
        Text(continent.name, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.ExtraBold, color = colors.text)
        Spacer(Modifier.weight(1f))
        Text("${continent.discovered}/${continent.available}", style = MaterialTheme.typography.bodyMedium, color = colors.muted)
    }
}

@Composable
fun CountryRow(progress: CountryProgress, info: Country?, onClick: () -> Unit) {
    val colors = LocalAppColors.current
    Row(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(10.dp))
            .clickable(onClick = onClick)
            .padding(8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(info?.flag.orEmpty(), fontSize = 20.sp, modifier = Modifier.width(36.dp))
        Text(info?.name ?: progress.code, color = colors.text, modifier = Modifier.weight(1f))
        Text("${progress.visited}/${progress.places}", fontSize = 13.sp, color = colors.muted)
        Box(Modifier.padding(start = 10.dp)) { StatusMark(progress.status) }
    }
}
