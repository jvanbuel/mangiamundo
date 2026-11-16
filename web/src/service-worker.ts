/// <reference types="@sveltejs/kit" />
/// <reference no-default-lib="true"/>
/// <reference lib="esnext" />
/// <reference lib="webworker" />

// Shows celebration and event reminders sent by the server, and opens the app on tap.
const sw = self as unknown as ServiceWorkerGlobalScope;

interface Reminder {
	title: string;
	body: string;
	url: string;
	tag: string;
}

sw.addEventListener('install', () => sw.skipWaiting());
sw.addEventListener('activate', (event) => event.waitUntil(sw.clients.claim()));

sw.addEventListener('push', (event) => {
	const reminder: Reminder = event.data?.json() ?? { title: 'Mangiamundo', body: '', url: '/', tag: 'mangiamundo' };
	event.waitUntil(
		sw.registration.showNotification(reminder.title, {
			body: reminder.body,
			tag: reminder.tag,
			icon: '/icon-192.png',
			badge: '/icon-192.png',
			data: { url: reminder.url }
		})
	);
});

sw.addEventListener('notificationclick', (event) => {
	event.notification.close();
	const url = new URL(event.notification.data?.url ?? '/', sw.location.origin).href;
	event.waitUntil(
		(async () => {
			const windows = await sw.clients.matchAll({ type: 'window', includeUncontrolled: true });
			const open = windows.find((w) => new URL(w.url).origin === sw.location.origin);
			if (open) {
				await open.navigate(url);
				return open.focus();
			}
			return sw.clients.openWindow(url);
		})()
	);
});
