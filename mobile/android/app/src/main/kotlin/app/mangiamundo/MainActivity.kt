package app.mangiamundo

import android.animation.ValueAnimator
import android.os.Bundle
import android.os.SystemClock
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import android.content.Intent
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import app.mangiamundo.core.StartupLoader
import app.mangiamundo.ui.AppScreen
import app.mangiamundo.ui.MangiamundoTheme

class MainActivity : ComponentActivity() {
    private val vm: AppViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        // The startup loader: plays all bites once, then stays until the first data is in.
        val splash = installSplashScreen()
        val started = SystemClock.uptimeMillis()
        splash.setKeepOnScreenCondition {
            val elapsed = if (ValueAnimator.areAnimatorsEnabled()) SystemClock.uptimeMillis() - started else Long.MAX_VALUE
            StartupLoader.keep(vm.ready, elapsed)
        }
        splash.setOnExitAnimationListener { provider ->
            provider.view.animate()
                .alpha(0f)
                .setDuration(if (ValueAnimator.areAnimatorsEnabled()) StartupLoader.FADE_MS else 0L)
                .withEndAction { provider.remove() }
                .start()
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

    /** A tapped reminder says which country or tab to open; news of new food has a link. */
    private fun handle(intent: Intent) {
        val country = intent.getStringExtra(Notifications.EXTRA_COUNTRY)
            ?: Push.countryIn(intent.getStringExtra(Push.EXTRA_URL))
        val badges = intent.getBooleanExtra(Notifications.EXTRA_BADGES, false)
        if (country != null || badges) vm.openFromNotification(country, badges)
    }
}
