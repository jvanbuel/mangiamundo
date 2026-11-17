import * as api from './api';
import * as push from './push';
import { hideSplash } from './splash';
import {
	statusOf,
	type BadgeKind,
	type Celebration,
	type Catalog,
	type Country,
	type Area,
	type Discover,
	type Located,
	type Location,
	type Place
} from './types';

const LOCATION_KEY = 'mangiamundo.location';
const PROJECTION_KEY = 'mangiamundo.projection';
const THEME_KEY = 'mangiamundo.theme';

export type Theme = 'system' | 'light' | 'dark';

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
	toasts: [] as Toast[]
});

/** The last real GPS position, used to verify check-ins even when browsing another city. */
let gpsFix: { lat: number; lng: number; at: number } | null = null;
let requestId = 0;
let toastId = 0;

export function country(code: string): Country | undefined {
	return app.catalog?.countries.find((c) => c.code === code);
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
	app.projection = load<'globe' | 'mercator'>(PROJECTION_KEY) ?? 'globe';
	app.theme = load<Theme>(THEME_KEY) ?? 'system';
	try {
		app.catalog = await api.catalog();
	} catch (error) {
		app.error = message(error);
		void hideSplash();
		return;
	}
	void checkReminders();
	const stored = await storedLocation();
	if (stored) {
		app.location = stored;
		await refresh();
		void hideSplash();
		// Keep following the device when the last location came from GPS.
		if (stored.source === 'gps') void locate(true);
	} else {
		// First visit: show the welcome card while the browser asks for the location.
		void hideSplash();
		await locate(true);
	}
	await followLink();
}

/** The saved location. One saved before areas existed (a point and a radius) is moved to
 * the city at that point. */
async function storedLocation(): Promise<Location | null> {
	const stored = load<Partial<Location> & { lat?: number; lng?: number; source?: 'gps' | 'search' }>(LOCATION_KEY);
	try {
		localStorage.removeItem('mangiamundo.radius');
	} catch {
		// Storage unavailable: nothing to clean up.
	}
	if (!stored) return null;
	if (stored.area && Array.isArray(stored.options)) return stored as Location;
	if (typeof stored.lat !== 'number' || typeof stored.lng !== 'number') return null;
	try {
		const located = await api.locateArea(stored.lat, stored.lng);
		const area = located.city ?? located.postal_code;
		if (!area) return null;
		const location: Location = {
			area,
			lat: stored.lat,
			lng: stored.lng,
			source: stored.source ?? 'search',
			options: optionsOf(located)
		};
		localStorage.setItem(LOCATION_KEY, JSON.stringify(location));
		return location;
	} catch {
		return null;
	}
}

function optionsOf(located: Located): Area[] {
	return [located.city, located.postal_code].filter((a): a is Area => a !== null);
}

/** Opens what a link (for example from a reminder) points at: ?country=JP or ?tab=badges. */
async function followLink() {
	const params = new URLSearchParams(location.search);
	if (!params.size) return;
	if (params.get('tab') === 'badges') app.tab = 'badges';
	const code = params.get('country')?.toUpperCase();
	if (code && app.discover?.countries.some((c) => c.code === code)) await select(code);
	history.replaceState(null, '', location.pathname);
}

async function checkReminders() {
	if (push.needsInstallForPush()) app.reminders = 'install';
	else if (!push.pushSupported()) app.reminders = 'unsupported';
	else app.reminders = (await push.currentSubscription()) && Notification.permission === 'granted' ? 'on' : 'off';
}

function pushTarget() {
	return { area: app.location!.area.id };
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
	// The old area's countries must not show while the new area loads.
	app.discover = null;
	app.celebrations = [];
	localStorage.setItem(LOCATION_KEY, JSON.stringify(location));
	await refresh();
}

/** Picks a city or postal code from a search. */
export async function chooseArea(area: Area) {
	await setLocation({ area, lat: area.lat, lng: area.lng, source: 'search', options: [] });
}

/** Switches between the city and the postal code found at the GPS position. */
export async function switchArea(area: Area) {
	const current = app.location;
	if (!current || current.area.id === area.id) return;
	await setLocation({ ...current, area });
}

/** Where the user is, when known recently: distances and check-ins are measured from it. */
function here(): { lat: number; lng: number } | null {
	if (gpsFix && Date.now() - gpsFix.at < 10 * 60_000) return { lat: gpsFix.lat, lng: gpsFix.lng };
	return app.location?.source === 'gps' ? { lat: app.location.lat, lng: app.location.lng } : null;
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
		// Background refreshes skip small moves; an explicit request always looks the area up,
		// since a short walk can cross a postal-code boundary.
		if (quiet && !moved) return;
		const located = await api.locateArea(coords.latitude, coords.longitude);
		// The city by default; someone who narrowed to their postal code keeps that. Read the
		// selection now: it may have changed while the lookup ran.
		const current = app.location;
		const keepPostalCode = current?.source === 'gps' && current.area.kind === 'postal_code';
		const area = (keepPostalCode && located.postal_code) || located.city || located.postal_code;
		if (!area) throw new Error('No city or postal code found here. Search for your city instead.');
		await setLocation({
			area,
			lat: coords.latitude,
			lng: coords.longitude,
			source: 'gps',
			options: optionsOf(located)
		});
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
		.celebrations(location.area.id)
		.then((list) => {
			if (app.location === location) app.celebrations = list;
		})
		.catch(() => (app.celebrations = []));
	const id = ++requestId;
	app.loading = true;
	app.error = null;
	try {
		const result = await api.discover(location.area.id);
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
		const result = await api.places(location.area.id, code, here());
		if (app.selected === code) app.places = result;
	} catch (error) {
		toast(null, 'Could not load places', message(error));
	} finally {
		if (app.selected === code) app.placesLoading = false;
	}
}

/** Announces everything a check-in unlocked by comparing progress before and after. */
function celebrate(before: Discover, after: Discover, place: Place) {
	for (const code of place.countries) {
		const was = before.countries.find((c) => c.code === code);
		const now = after.countries.find((c) => c.code === code);
		const info = country(code);
		if (!was || !now || !info) continue;
		if (statusOf(now) === 'completed' && statusOf(was) !== 'completed') {
			toast({ text: info.flag }, `${info.name} complete`, `You visited every place in ${app.location?.area.name ?? 'this area'}.`);
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

export async function toggleVisit(place: Place) {
	const before = app.discover;
	const location = app.location;
	if (!before || !location) return;
	const visiting = place.visited_at === null;
	try {
		if (visiting) {
			const fresh = gpsFix && Date.now() - gpsFix.at < 10 * 60_000 ? gpsFix : null;
			const result = await api.checkIn(place.id, fresh && { lat: fresh.lat, lng: fresh.lng });
			place.visited_at = Math.floor(Date.now() / 1000);
			place.verified = result.verified;
		} else {
			await api.undoCheckIn(place.id);
			place.visited_at = null;
			place.verified = false;
		}
		const after = await api.discover(location.area.id);
		app.discover = after;
		if (visiting) celebrate(before, after, place);
	} catch (error) {
		toast(null, 'That did not work', message(error));
	}
}
