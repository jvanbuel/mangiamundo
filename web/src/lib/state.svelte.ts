import * as api from './api';
import {
	statusOf,
	type BadgeKind,
	type Catalog,
	type Country,
	type Discover,
	type Location,
	type Place
} from './types';

const LOCATION_KEY = 'mangiamundo.location';
const RADIUS_KEY = 'mangiamundo.radius';
const PROJECTION_KEY = 'mangiamundo.projection';
const THEME_KEY = 'mangiamundo.theme';

export type Theme = 'system' | 'light' | 'dark';

export const RADII = [5, 10, 25];

/** What a toast shows on its left: a flag (or other text), a badge medal, or nothing. */
export type ToastIcon = { text: string } | { badge: BadgeKind } | null;

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
	app.radiusKm = load<number>(RADIUS_KEY) ?? 10;
	app.projection = load<'globe' | 'mercator'>(PROJECTION_KEY) ?? 'globe';
	app.theme = load<Theme>(THEME_KEY) ?? 'system';
	try {
		app.catalog = await api.catalog();
	} catch (error) {
		app.error = message(error);
		return;
	}
	const stored = load<Location>(LOCATION_KEY);
	if (stored) {
		app.location = stored;
		await refresh();
		// Keep following the device when the last location came from GPS.
		if (stored.source === 'gps') void locate(true);
	} else {
		await locate(true);
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
function celebrate(before: Discover, after: Discover, place: Place) {
	for (const code of place.countries) {
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
		const after = await api.discover(location.lat, location.lng, app.radiusKm);
		app.discover = after;
		if (visiting) celebrate(before, after, place);
	} catch (error) {
		toast(null, 'That did not work', message(error));
	}
}
