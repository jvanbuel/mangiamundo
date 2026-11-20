import * as api from './api';

/** Web Push needs a service worker and the Push API; iPhones only offer it to installed apps. */
export function pushSupported(): boolean {
	return 'serviceWorker' in navigator && 'PushManager' in window && 'Notification' in window;
}

/** True on iPhone or iPad Safari before the app is added to the home screen. */
export function needsInstallForPush(): boolean {
	const ios = /iPad|iPhone|iPod/.test(navigator.userAgent);
	const standalone = window.matchMedia('(display-mode: standalone)').matches;
	return ios && !standalone;
}

function keyBytes(base64url: string): Uint8Array<ArrayBuffer> {
	const base64 = base64url.replace(/-/g, '+').replace(/_/g, '/');
	const raw = atob(base64 + '='.repeat((4 - (base64.length % 4)) % 4));
	return Uint8Array.from(raw, (c) => c.charCodeAt(0));
}

async function registration(): Promise<ServiceWorkerRegistration> {
	return (await navigator.serviceWorker.getRegistration()) ?? navigator.serviceWorker.register('/service-worker.js', { type: 'module' });
}

export async function currentSubscription(): Promise<PushSubscription | null> {
	if (!pushSupported()) return null;
	return (await (await registration()).pushManager.getSubscription()) ?? null;
}

/** Asks for permission if needed, subscribes, and tells the server where the user is and
 * which notifications they want. */
export async function enablePush(target: Omit<api.PushTarget, 'timezone'>): Promise<void> {
	if (!pushSupported()) throw new Error('This browser cannot show notifications.');
	const permission = await Notification.requestPermission();
	if (permission !== 'granted') throw new Error('Notifications are blocked for this site.');
	const reg = await registration();
	const subscription =
		(await reg.pushManager.getSubscription()) ??
		(await reg.pushManager.subscribe({
			userVisibleOnly: true,
			applicationServerKey: keyBytes((await api.pushKey()).public_key)
		}));
	await api.savePushSubscription(subscription.toJSON(), {
		...target,
		timezone: Intl.DateTimeFormat().resolvedOptions().timeZone
	});
}

/** Keeps the server's copy in step after the user moves to another area. */
export async function updatePush(target: Omit<api.PushTarget, 'timezone'>): Promise<void> {
	const subscription = await currentSubscription();
	if (!subscription || Notification.permission !== 'granted') return;
	await api.savePushSubscription(subscription.toJSON(), {
		...target,
		timezone: Intl.DateTimeFormat().resolvedOptions().timeZone
	});
}

export async function disablePush(): Promise<void> {
	const subscription = await currentSubscription();
	if (!subscription) return;
	await api.deletePushSubscription(subscription.endpoint).catch(() => undefined);
	await subscription.unsubscribe();
}
