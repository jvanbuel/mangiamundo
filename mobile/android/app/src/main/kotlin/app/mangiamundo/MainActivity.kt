package app.mangiamundo

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import app.mangiamundo.ui.AppScreen
import app.mangiamundo.ui.MangiamundoTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            MangiamundoTheme {
                AppScreen()
            }
        }
    }
}
