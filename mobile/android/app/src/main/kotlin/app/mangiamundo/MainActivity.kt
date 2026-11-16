package app.mangiamundo

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import android.content.Intent
import app.mangiamundo.ui.AppScreen
import app.mangiamundo.ui.MangiamundoTheme

class MainActivity : ComponentActivity() {
    private val vm: AppViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        if (savedInstanceState == null) handle(intent)
        setContent {
            MangiamundoTheme {
                AppScreen(vm)
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handle(intent)
    }

    /** A tapped reminder says which country or tab to open. */
    private fun handle(intent: Intent) {
        val country = intent.getStringExtra(Notifications.EXTRA_COUNTRY)
        val badges = intent.getBooleanExtra(Notifications.EXTRA_BADGES, false)
        if (country != null || badges) vm.openFromNotification(country, badges)
    }
}
