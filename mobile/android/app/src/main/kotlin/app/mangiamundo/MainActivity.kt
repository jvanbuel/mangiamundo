package app.mangiamundo

import android.os.Bundle
import android.os.SystemClock
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import android.content.Intent
import app.mangiamundo.ui.AppScreen
import app.mangiamundo.ui.MangiamundoTheme

class MainActivity : ComponentActivity() {
    private val vm: AppViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        // Keep the startup loader up until the first data is in, but never cut a bite off halfway.
        val shown = SystemClock.uptimeMillis()
        installSplashScreen().setKeepOnScreenCondition {
            val t = SystemClock.uptimeMillis() - shown
            !vm.ready || BITES.any { t in it }
        }
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

    private companion object {
        /** When each bite grows, in ms: the same times as res/drawable/splash_animated.xml. */
        val BITES = listOf(200L..350L, 470L..620L, 740L..890L)
    }

    /** A tapped reminder says which country or tab to open. */
    private fun handle(intent: Intent) {
        val country = intent.getStringExtra(Notifications.EXTRA_COUNTRY)
        val badges = intent.getBooleanExtra(Notifications.EXTRA_BADGES, false)
        if (country != null || badges) vm.openFromNotification(country, badges)
    }
}
