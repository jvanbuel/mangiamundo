import * as api from './api';
import * as push from './push';
import {
	statusOf,
	type BadgeKind,
	type Celebration,
	type Catalog,
	type Country,
	type Discover,
	type Dish,
	type Location,
	type Place
} from './types';

const LOCATION_KEY = 'mangiamundo.location';
const RADIUS_KEY = 'mangiamundo.radius';
const PROJECTION_KEY = 'mangiamundo.projection';
const THEME_KEY = 'mangiamundo.theme';

export type Theme = 'system' | 'light' | 'dark';

export const RADII = [5, 10, 25];

/** Mirrors the server's rules, for optimistic updates and messages. */
const CHECKIN_COOLDOWN_SECS = 12 * 3600;
const XP_PER_VISIT = 10;
const XP_PER_DISH = 25;

/** What a toast shows on its left: a flag (or other text), a badge medal, or nothing. */
export type ToastIcon = { text: string } | { badge: BadgeKind | 'event' } | null;

export interface Toast {
	id: number;
	icon: ToastIcon;
	title: string;
	text: string;
}

function load<T>(key: string): T | null {
	try {
		return JSON.parse(localStorage.getItem(key) ?? 'null');
	} catch {
		return null;
	}
}

export const app = $state({
	catalog: null as Catalog | null,
	location: null as Location | null,
	radiusKm: 10,
	discover: null as Discover | null,
	loading: false,
	error: null as string | null,
	/** Code of the country whose places are shown. */
	selected: null as string | null,
	places: [] as Place[],
	placesLoading: false,
	locating: false,
	locateError: null as string | null,
	projection: 'globe' as 'globe' | 'mercator',
	theme: 'system' as Theme,
	tab: 'countries' as 'countries' | 'badges',
	celebrations: [] as Celebration[],
	/** Web Push reminders: on, off, not possible here, or possible once installed (iPhone). */
	reminders: 'off' as 'on' | 'off' | 'unsupported' | 'install',
	remindersBusy: false,
	toasts: [] as Toast[],
	/** Which dialog is open. */
	dialog: null as 'share' | 'backup' | null,
	/** A recovery code that arrived in a link, waiting for the user to confirm. */
	pendingRestore: null as string | null
});

/** The last real GPS position, used to verify check-ins even when browsing another city. */
let gpsFix: { lat: number; lng: number; at: number } | null = null;
let requestId = 0;
let toastId = 0;

export function country(code: string): Country | undefined {
	return app.catalog?.countries.find((c) => c.code === code);
}

/** Signature dishes of the given countries, in catalog order. */
export function dishesOf(codes: string[]): Dish[] {
	return app.catalog?.dishes.filter((d) => codes.includes(d.country)) ?? [];
}

export function toast(icon: ToastIcon, title: string, text: string) {
	const id = ++toastId;
	app.toasts.push({ id, icon, title, text });
	setTimeout(() => {
		app.toasts = app.toasts.filter((t) => t.id !== id);
	}, 5000);
}

function message(error: unknown): string {
	return error instanceof Error ? error.message : 'Something went wrong.';
}

export async function init() {
	app.radiusKm = load<number>(RADIUS_KEY) ?? 10;
	app.projection = load<'globe' | 'mercator'>(PROJECTION_KEY) ?? 'globe';
	app.theme = load<Theme>(THEME_KEY) ?? 'system';
	try {
		app.catalog = await api.catalog();
	} catch (error) {
		app.error = message(error);
		return;
	}
	void checkReminders();
	const stored = load<Location>(LOCATION_KEY);
	if (stored) {
		app.location = stored;
		await refresh();
		// Keep following the device when the last location came from GPS.
		if (stored.source === 'gps') void locate(true);
	} else {
		await locate(true);
	}
	await followLink();
}

/**
 * Opens what a link (for example from a reminder) points at: ?country=JP or ?tab=badges.
 * ?restore=CODE comes from the backup QR code and asks before switching passports.
 */
async function followLink() {
	const params = new URLSearchParams(location.search);
	if (!params.size) return;
	if (params.get('tab') === 'badges') app.tab = 'badges';
	const code = params.get('restore');
	if (code) {
		app.pendingRestore = code;
		app.dialog = 'backup';
	}
	const country = params.get('country')?.toUpperCase();
	if (country && app.discover?.countries.some((c) => c.code === country)) await select(country);
	history.replaceState(null, '', location.pathname);
}

async function checkReminders() {
	if (push.needsInstallForPush()) app.reminders = 'install';
	else if (!push.pushSupported()) app.reminders = 'unsupported';
	else app.reminders = (await push.currentSubscription()) && Notification.permission === 'granted' ? 'on' : 'off';
}

function pushTarget() {
	const here = app.location!;
	return { lat: here.lat, lng: here.lng, radius_km: app.radiusKm };
}

export async function toggleReminders() {
	if (!app.location || app.remindersBusy) return;
	app.remindersBusy = true;
	try {
		if (app.reminders === 'on') {
			await push.disableReminders();
			app.reminders = 'off';
		} else {
			await push.enableReminders(pushTarget());
			app.reminders = 'on';
			toast(null, 'Reminders on', 'We will let you know when a country near you has something to celebrate.');
		}
	} catch (error) {
		toast(null, 'Could not change reminders', message(error));
	} finally {
		app.remindersBusy = false;
	}
}

export function setProjection(projection: 'globe' | 'mercator') {
	app.projection = projection;
	localStorage.setItem(PROJECTION_KEY, JSON.stringify(projection));
}

export function setTheme(theme: Theme) {
	// The attribute goes first so anything reacting to `app.theme` already sees the new colours.
	if (theme === 'system') delete document.documentElement.dataset.theme;
	else document.documentElement.dataset.theme = theme;
	localStorage.setItem(THEME_KEY, JSON.stringify(theme));
	app.theme = theme;
}

export async function setLocation(location: Location) {
	app.location = location;
	app.selected = null;
	app.places = [];
	localStorage.setItem(LOCATION_KEY, JSON.stringify(location));
	await refresh();
}

export async function setRadius(radiusKm: number) {
	app.radiusKm = radiusKm;
	localStorage.setItem(RADIUS_KEY, JSON.stringify(radiusKm));
	app.selected = null;
	await refresh();
}

function position(): Promise<GeolocationPosition> {
	return new Promise((resolve, reject) => {
		if (!('geolocation' in navigator)) {
			reject(new Error('This device cannot share its location.'));
			return;
		}
		navigator.geolocation.getCurrentPosition(resolve, (e) => reject(new Error(
			e.code === e.PERMISSION_DENIED
				? 'Location access was denied. Search for your city instead.'
				: 'Could not determine your location. Search for your city instead.'
		)), { enableHighAccuracy: true, timeout: 10_000, maximumAge: 60_000 });
	});
}

/** Moves to the device's GPS position. With `quiet`, failures leave the current location alone. */
export async function locate(quiet = false) {
	app.locating = true;
	app.locateError = null;
	try {
		const { coords } = await position();
		gpsFix = { lat: coords.latitude, lng: coords.longitude, at: Date.now() };
		const previous = app.location;
		const moved =
			!previous ||
			previous.source !== 'gps' ||
			Math.abs(previous.lat - coords.latitude) > 0.01 ||
			Math.abs(previous.lng - coords.longitude) > 0.01;
		if (!moved) return;
		const name = await api
			.reverseGeocode(coords.latitude, coords.longitude)
			.then((r) => r.name)
			.catch(() => null);
		await setLocation({ lat: coords.latitude, lng: coords.longitude, name, source: 'gps' });
	} catch (error) {
		if (!quiet || !app.location) app.locateError = message(error);
	} finally {
		app.locating = false;
	}
}

export async function refresh() {
	const location = app.location;
	if (!location) return;
	if (app.reminders === 'on') void push.updateReminders(pushTarget()).catch(() => undefined);
	void api
		.celebrations(location.lat, location.lng, app.radiusKm)
		.then((list) => {
			if (app.location === location) app.celebrations = list;
		})
		.catch(() => (app.celebrations = []));
	const id = ++requestId;
	app.loading = true;
	app.error = null;
	try {
		const result = await api.discover(location.lat, location.lng, app.radiusKm);
		if (id === requestId) app.discover = result;
	} catch (error) {
		if (id === requestId) {
			app.error = message(error);
			app.discover = null;
		}
	} finally {
		if (id === requestId) app.loading = false;
	}
}

export async function select(code: string | null) {
	app.selected = code;
	app.places = [];
	const location = app.location;
	if (!code || !location) return;
	app.placesLoading = true;
	try {
		const result = await api.places(location.lat, location.lng, app.radiusKm, code);
		if (app.selected === code) app.places = result;
	} catch (error) {
		toast(null, 'Could not load places', message(error));
	} finally {
		if (app.selected === code) app.placesLoading = false;
	}
}

/** Announces everything a check-in unlocked by comparing progress before and after. */
function celebrate(before: Discover, after: Discover, place: Place | null) {
	for (const code of place?.countries ?? []) {
		const was = before.countries.find((c) => c.code === code);
		const now = after.countries.find((c) => c.code === code);
		const info = country(code);
		if (!was || !now || !info) continue;
		if (statusOf(now) === 'completed' && statusOf(was) !== 'completed') {
			toast({ text: info.flag }, `${info.name} complete`, 'You visited every place in this city.');
		} else if (was.visited === 0) {
			toast({ text: info.flag }, `${info.name} tasted`, `${now.places - now.visited} more to complete it.`);
		}
	}
	for (const badge of after.badges) {
		if (badge.earned && !before.badges.find((b) => b.id === badge.id)?.earned) {
			toast({ badge: badge.kind }, `Badge: ${badge.title}`, badge.description);
		}
	}
	for (const event of after.events) {
		if (event.earned && !before.events.find((e) => e.id === event.id)?.earned) {
			toast({ badge: 'event' }, `Badge: ${event.title}`, 'Earned while it lasted.');
		}
	}
	if (after.profile.level > before.profile.level) {
		toast({ text: String(after.profile.level) }, `Level ${after.profile.level}`, `${after.profile.xp} XP so far.`);
	}
}

/** Rereads progress after a change and announces what it unlocked. */
async function afterChange(before: Discover, place: Place | null) {
	const location = app.location;
	if (!location) return;
	const after = await api.discover(location.lat, location.lng, app.radiusKm);
	app.discover = after;
	celebrate(before, after, place);
}

/** Checks in at a place: the first visit, or a return once the cooldown has passed. */
export async function checkIn(place: Place) {
	const before = app.discover;
	if (!before) return;
	try {
		const fresh = gpsFix && Date.now() - gpsFix.at < 10 * 60_000 ? gpsFix : null;
		const result = await api.checkIn(place.id, fresh && { lat: fresh.lat, lng: fresh.lng });
		const now = Math.floor(Date.now() / 1000);
		place.visited_at ??= now;
		place.last_visited_at = now;
		place.visits += 1;
		place.verified ||= result.verified;
		place.last_checkin_id = result.checkin_id;
		place.last_dish = null;
		place.next_check_in_at = now + CHECKIN_COOLDOWN_SECS;
		if (place.visits > 1) {
			toast({ text: `${place.visits}×` }, `Back at ${place.name}`, `+${XP_PER_VISIT} XP. What did you have?`);
		}
		await afterChange(before, place);
	} catch (error) {
		toast(null, 'That did not work', message(error));
	}
}

/** Logs the signature dish eaten at the place's latest check-in, or clears it with null. */
export async function logDish(place: Place, dish: Dish | null) {
	const before = app.discover;
	if (!before || place.last_checkin_id === null) return;
	const previous = place.last_dish;
	place.last_dish = dish?.id ?? null;
	try {
		await api.setDish(place.last_checkin_id, place.last_dish);
		await afterChange(before, null);
		if (dish && !before.dishes_tried.includes(dish.id)) {
			toast({ text: country(dish.country)?.flag ?? '' }, `${dish.name} tasted`, `+${XP_PER_DISH} XP for your dish passport.`);
		}
	} catch (error) {
		place.last_dish = previous;
		toast(null, 'That did not work', message(error));
	}
}

/** Takes back the place's latest check-in. */
export async function undoCheckIn(place: Place) {
	const before = app.discover;
	if (!before || place.last_checkin_id === null) return;
	try {
		await api.undoCheckIn(place.last_checkin_id);
		// The places list knows what the previous check-in was; ask for it again.
		if (app.selected) {
			const location = app.location!;
			const fresh = await api.places(location.lat, location.lng, app.radiusKm, app.selected);
			const updated = fresh.find((p) => p.id === place.id);
			if (updated) Object.assign(place, updated);
		}
		await afterChange(before, null);
	} catch (error) {
		toast(null, 'That did not work', message(error));
	}
}

/** Switches this device to the passport behind a recovery code. */
export async function restorePassport(code: string): Promise<boolean> {
	try {
		const merged = await api.restore(code);
		app.selected = null;
		app.places = [];
		await refresh();
		toast(
			{ badge: 'visits' },
			'Passport restored',
			merged ? `${merged} check-ins from this device came along.` : 'Welcome back.'
		);
		return true;
	} catch (error) {
		const text = error instanceof api.ApiError && error.status === 404 ? 'That code does not match a passport.' : message(error);
		toast(null, 'Could not restore', text);
		return false;
	}
}
