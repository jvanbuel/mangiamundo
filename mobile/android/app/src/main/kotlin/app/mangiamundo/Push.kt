package app.mangiamundo

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import app.mangiamundo.core.ApiClient
import com.google.firebase.FirebaseApp
import com.google.firebase.messaging.FirebaseMessaging
import com.google.firebase.messaging.FirebaseMessagingService
import com.google.firebase.messaging.RemoteMessage
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

/**
 * News of food the user hasn't tasted showing up nearby. Unlike celebrations these can't be
 * known in advance, so the server sends them through Firebase Cloud Messaging.
 *
 * Firebase is configured by `app/google-services.json`. A build without it has no Firebase
 * and `available` is false.
 */
object Push {
    /** Must match the server's `fcm::CHANNEL`. */
    const val CHANNEL = "new_food"

    /** The notification's link, put in the tap intent by FCM (background) or by `show`. */
    const val EXTRA_URL = "url"

    fun available(context: Context): Boolean = FirebaseApp.getApps(context).isNotEmpty()

    /** The channel FCM shows these notifications in; it must exist before the first one. */
    fun createChannel(context: Context) {
        context.getSystemService(NotificationManager::class.java).createNotificationChannel(
            NotificationChannel(CHANNEL, "New food near you", NotificationManager.IMPORTANCE_DEFAULT).apply {
                description = "Food from a country you haven't tasted yet shows up in your area"
            },
        )
    }

    suspend fun token(): String = suspendCancellableCoroutine { continuation ->
        FirebaseMessaging.getInstance().token.addOnCompleteListener { task ->
            if (task.isSuccessful) continuation.resume(task.result)
            else continuation.resumeWithException(task.exception ?: IllegalStateException("No push token"))
        }
    }

    /** Forgets the token, so Firebase stops delivering to this install. */
    fun deleteToken() {
        FirebaseMessaging.getInstance().deleteToken()
    }

    /** The country in a link such as `/?country=GE`. */
    fun countryIn(url: String?): String? =
        url?.let { runCatching { Uri.parse("https://x$it").getQueryParameter("country") }.getOrNull() }

    /** Shows a message that arrived while the app was in the foreground, where FCM doesn't. */
    fun show(context: Context, title: String, body: String, url: String?, tag: String?) {
        if (!Notifications.allowed(context)) return
        createChannel(context)
        val open = Intent(context, MainActivity::class.java)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
            .putExtra(EXTRA_URL, url)
        val id = (tag ?: title).hashCode()
        val tap = PendingIntent.getActivity(context, id, open, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        val notification = NotificationCompat.Builder(context, CHANNEL)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle(title)
            .setContentText(body)
            .setStyle(NotificationCompat.BigTextStyle().bigText(body))
            .setContentIntent(tap)
            .setAutoCancel(true)
            .build()
        try {
            NotificationManagerCompat.from(context).notify(tag, id, notification)
        } catch (_: SecurityException) {
            // Permission was withdrawn.
        }
    }
}

class MessagingService : FirebaseMessagingService() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    /** A new token replaces the old one at the server, if the user opted in. */
    override fun onNewToken(token: String) {
        val settings = Settings(applicationContext)
        val area = settings.location?.area?.id ?: return
        if (!settings.newFoodOn) return
        scope.launch {
            runCatching { ApiClient(BuildConfig.API_BASE, settings).registerDevice(token, area) }
        }
    }

    override fun onMessageReceived(message: RemoteMessage) {
        val notification = message.notification ?: return
        Push.show(
            applicationContext,
            notification.title ?: return,
            notification.body.orEmpty(),
            message.data[Push.EXTRA_URL],
            notification.tag,
        )
    }
}
