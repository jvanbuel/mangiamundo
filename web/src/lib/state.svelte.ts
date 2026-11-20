import { tick } from 'svelte';
import * as api from './api';
import * as push from './push';
import { hideSplash } from './splash';
import {
	statusOf,
	type AddPlace,
	type AddedPlace,
	type Bonus,
	type BadgeKind,
	type Celebration,
	type Catalog,
	type Country,
	type Area,
	type AreaProgress,
	type Discover,
	type Located,
	type Location,
	type Place,
	type Visit,
	visitedToday
} from './types';

const LOCATION_KEY = 'mangiamundo.location';
const PROJECTION_KEY = 'mangiamundo.projection';
const THEME_KEY = 'mangiamundo.theme';
const RECENTS_KEY = 'mangiamundo.recentAreas';
/** Areas kept as recent, including the current one; the picker shows the others. */
const MAX_RECENTS = 6;

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
	/** The server's version and commit; null until it answers, 'not reached' after a failure. */
	server: null as string | null,
	catalog: null as Catalog | null,
	location: null as Location | null,
	discover: null as Discover | null,
	loading: false,
	/** How far the area's restaurants have loaded, while that takes a while. */
	areaProgress: null as AreaProgress | null,
	error: null as string | null,
	/** Code of the country whose places are shown. */
	selected: null as string | null,
	places: [] as Place[],
	placesLoading: false,
	/** Id of the place whose details are open, and the details once loaded. */
	placeId: null as string | null,
	detail: null as Place | null,
	/** The last place whose details closed, so its link in the list gets focus back. */
	closedPlace: null as string | null,
	/** The place whose check-in or undo is running. One at a time, across cards and details. */
	busyPlace: null as string | null,
	/** The place being read again from OpenStreetMap. */
	refreshingPlace: null as string | null,
	/** Asking OpenStreetMap for places just mapped nearby. */
	lookingNearby: false,
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
	/** Areas used lately, newest first, for switching back quickly. */
	recents: [] as Area[]
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

/** Asks for the server's version until it answers once. */
function loadServer() {
	if (app.server && app.server !== 'not reached') return;
	api
		.health()
		.then((h) => (app.server = [h.version ?? 'unknown', h.commit].filter(Boolean).join(' ')))
		.catch(() => (app.server = 'not reached'));
}

export async function init() {
	loadServer();
	app.projection = load<'globe' | 'mercator'>(PROJECTION_KEY) ?? 'globe';
	app.theme = load<Theme>(THEME_KEY) ?? 'system';
	app.recents = load<Area[]>(RECENTS_KEY) ?? [];
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

/** Opens what a link (for example from a reminder) points at: ?country=JP, ?tab=badges or
 * ?place=osm-node-123. A place link stays in the address bar while its details are open. */
async function followLink() {
	// Back from editing a place on OpenStreetMap: show the edit now, not after the next import.
	document.addEventListener('visibilitychange', () => {
		if (document.visibilityState !== 'visible' || !editing) return;
		const id = editing;
		editing = null;
		// A refresh still running may predate the edit: this one goes after it.
		if (app.refreshingPlace) queuedRefresh = id;
		else void refreshPlace(id, true);
	});
	addEventListener('popstate', () => {
		closing = false;
		const id = new URLSearchParams(location.search).get('place');
		if (id) void loadPlace(id);
		else clearPlace();
	});
	const params = new URLSearchParams(location.search);
	if (!params.size) return;
	if (params.get('tab') === 'badges') app.tab = 'badges';
	const code = params.get('country')?.toUpperCase();
	if (code && app.discover?.countries.some((c) => c.code === code)) await select(code);
	const id = params.get('place');
	history.replaceState(null, '', id ? `?place=${encodeURIComponent(id)}` : location.pathname);
	if (id) await loadPlace(id);
}

/** Opens a place's details, as a new history entry so Back closes them. */
export async function openPlace(id: string) {
	history.pushState({ place: id }, '', `?place=${encodeURIComponent(id)}`);
	await loadPlace(id);
}

/** Closes the details: back to where they were opened from, or to the list after a link. */
export function closePlace() {
	if (history.state?.place) {
		// Once per close: a second Back before popstate would leave the list too.
		if (closing) return;
		closing = true;
		loads++;
		history.back();
	} else {
		clearPlace();
		history.replaceState(null, '', location.pathname);
	}
}

function clearPlace() {
	if (app.placeId) {
		app.closedPlace = app.placeId;
		// Only the list that renders right now may take focus.
		void tick().then(() => (app.closedPlace = null));
	}
	app.placeId = null;
	app.detail = null;
	loads++;
}

/** Counts detail loads, so only the latest one shows its result. */
let loads = 0;
/** A Back from the details is on its way. */
let closing = false;

async function loadPlace(id: string) {
	const load = ++loads;
	app.placeId = id;
	app.detail = null;
	// Measured from the user when known, otherwise from the map pin, as the list is.
	const from = here() ?? (app.location && { lat: app.location.lat, lng: app.location.lng });
	try {
		const place = await api.place(id, from);
		if (load === loads) app.detail = place;
	} catch (error) {
		if (load !== loads) return;
		toast(null, 'Could not load this place', message(error));
		closePlace();
	}
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
	// The area being left counts too, so a location saved before recents existed is kept.
	const left = app.location ? remembering(app.location.area, app.recents) : app.recents;
	app.location = location;
	saveRecents(remembering(location.area, left));
	app.selected = null;
	app.places = [];
	// The old area's countries must not show while the new area loads.
	app.discover = null;
	app.celebrations = [];
	localStorage.setItem(LOCATION_KEY, JSON.stringify(location));
	await refresh();
}

/** `recents` with `area` in front, once, and at most MAX_RECENTS areas. */
function remembering(area: Area, recents: Area[]): Area[] {
	return [area, ...recents.filter((a) => a.id !== area.id)].slice(0, MAX_RECENTS);
}

function saveRecents(recents: Area[]) {
	app.recents = recents;
	try {
		localStorage.setItem(RECENTS_KEY, JSON.stringify(recents));
	} catch {
		// Storage unavailable: recents last until the page closes.
	}
}

/** Removes an area from the recent ones. */
export function forgetRecent(id: string) {
	saveRecents(app.recents.filter((a) => a.id !== id));
}

/** Picks a city or postal code from a search. */
export async function chooseArea(area: Area) {
	if (app.placeId) closePlace();
	await setLocation({ area, lat: area.lat, lng: area.lng, source: 'search', options: [] });
}

/** Switches between the city and the postal code found at the GPS position. */
export async function switchArea(area: Area) {
	const current = app.location;
	if (!current || current.area.id === area.id) return;
	if (app.placeId) closePlace();
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
		// A background move leaves open details alone: a place doesn't depend on the area.
		if (!quiet && app.placeId) closePlace();
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
	loadServer();
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
	// A wheel from the previous area must not linger while this one starts.
	app.areaProgress = null;
	const stopProgress = watchProgress(location.area.id, id);
	try {
		const result = await api.discover(location.area.id);
		if (id === requestId) app.discover = result;
		void announceConfirmedAdditions();
	} catch (error) {
		if (id === requestId) {
			app.error = message(error);
			app.discover = null;
		}
	} finally {
		stopProgress();
		if (id === requestId) app.loading = false;
	}
}

/** Polls an area's load progress while discover waits on it, one request at a time. Shown
 * only after 300 ms, so areas that answer quickly don't flash it, and only for the latest
 * refresh (`id`). Returns a function that stops and hides it. */
function watchProgress(areaId: string, id: number): () => void {
	const started = Date.now();
	let stopped = false;
	void (async () => {
		while (!stopped) {
			await new Promise((resolve) => setTimeout(resolve, 400));
			if (stopped) return;
			const progress = await api.areaProgress(areaId).catch(() => null);
			// null between steps or before the server starts: keep what is shown.
			if (stopped || id !== requestId || !progress || Date.now() - started < 300) continue;
			app.areaProgress = progress;
			// At startup the splash waits for the data: let the wheel show instead.
			void hideSplash();
		}
	})();
	return () => {
		stopped = true;
		if (id === requestId) app.areaProgress = null;
	};
}

export async function select(code: string | null) {
	// Picking a country on the map while a place is open shows that country's list.
	if (app.placeId && code) closePlace();
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
	if (!before || !location || app.busyPlace) return;
	// Visited today: undo that visit. Otherwise check in, also when there are older visits.
	const undoing = visitedToday(place);
	app.busyPlace = place.id;
	try {
		if (!undoing) {
			const fresh = gpsFix && Date.now() - gpsFix.at < 10 * 60_000 ? gpsFix : null;
			const result = await api.checkIn(place.id, fresh && { lat: fresh.lat, lng: fresh.lng });
			setVisits(place.id, (visits) => [result.visit, ...visits.filter((v) => v.id !== result.visit.id)]);
		} else {
			const removed = place.visits[0].id;
			await api.removeVisit(removed);
			setVisits(place.id, (visits) => visits.filter((v) => v.id !== removed));
		}
		// Details still loading may have read the visits from before this change.
		if (app.placeId === place.id && !app.detail) void loadPlace(place.id);
		if (undoing) {
			// The server has the remaining visits; details shown here may be out of date.
			const rest = await api.place(place.id, null).then((p) => p.visits, () => null);
			if (rest) setVisits(place.id, () => rest);
			else if (app.detail?.id === place.id) {
				// The card shows only the latest visit: fall back on the older ones in the details.
				const card = app.places.find((p) => p.id === place.id);
				if (card) card.visits = app.detail.visits.slice(0, 1);
			}
		}
		const after = await api.discover(location.area.id);
		app.discover = after;
		if (!undoing) celebrate(before, after, place);
	} catch (error) {
		toast(null, 'That did not work', message(error));
	} finally {
		app.busyPlace = null;
	}
}

/** Reports a place as closed for good: it disappears from this user's lists right away. */
export async function reportClosed(place: Place) {
	if (app.busyPlace) return;
	app.busyPlace = place.id;
	try {
		await api.reportClosed(place.id);
		app.places = app.places.filter((p) => p.id !== place.id);
		if (app.placeId === place.id) closePlace();
		toast(null, 'Thanks for letting us know', `${place.name} is hidden for you now.`);
		// The place no longer counts towards its countries.
		const location = app.location;
		if (location) app.discover = await api.discover(location.area.id);
	} catch (error) {
		toast(null, 'Could not report this place', message(error));
	} finally {
		app.busyPlace = null;
	}
}

/** The place whose OpenStreetMap page the user opened to edit it, read again when they come back. */
let editing: string | null = null;
/** A refresh waiting for the one running to finish. */
let queuedRefresh: string | null = null;

export function editingOnOsm(id: string) {
	editing = id;
}

/** Reads a place again from OpenStreetMap and shows what changed. With `quiet`, a failure is
 * not shown, as when it runs by itself after the user comes back from editing. */
export async function refreshPlace(id: string, quiet = false) {
	if (app.refreshingPlace) return;
	app.refreshingPlace = id;
	const name = app.detail?.id === id ? app.detail.name : (app.places.find((p) => p.id === id)?.name ?? 'This place');
	try {
		const { status, place } = await api.refreshPlace(id, here());
		if (status === 'gone' || !place) {
			app.places = app.places.filter((p) => p.id !== id);
			if (app.placeId === id) closePlace();
			toast(null, `${name} is gone from OpenStreetMap`, 'It is hidden now. Your visits to it are kept.');
		} else {
			// Also when unchanged: someone else's refresh may have updated it since it loaded.
			applyRefreshed(place);
			// Details reopened while this ran are still loading, from before the refresh was
			// saved: load them again now, before anything else can finish first.
			if (app.placeId === id && !app.detail) void loadPlace(id);
			if (status === 'updated') toast(null, `${place.name} updated`, 'This is the latest from OpenStreetMap.');
			else toast(null, 'No change on OpenStreetMap yet', 'A new edit can take a minute or two to show up there. Try again shortly.');
		}
		// Its countries may have changed, also with an unchanged reply when someone else's
		// refresh changed it first. A reply for an area the user has left is dropped.
		const location = app.location;
		if (location) {
			const discover = await api.discover(location.area.id);
			if (app.location === location) app.discover = discover;
		}
	} catch (error) {
		if (!quiet) toast(null, 'Could not refresh this place', message(error));
	} finally {
		app.refreshingPlace = null;
		const next = queuedRefresh;
		queuedRefresh = null;
		if (next) void refreshPlace(next, true);
	}
}

/** Shows a place read again from OpenStreetMap. Visits stay as shown: a check-in or undo
 * made while the refresh ran is newer than the reply. A place no longer serving the selected
 * country's food leaves that list. */
function applyRefreshed(place: Place) {
	if (app.detail?.id === place.id) app.detail = { ...place, visits: app.detail.visits };
	app.places = app.places
		.map((p) => (p.id === place.id ? { ...place, visits: p.visits } : p))
		.filter((p) => p.id !== place.id || !app.selected || p.countries.includes(app.selected));
}

/** Asks OpenStreetMap for restaurants just mapped around the user, for someone who added one
 * in an OpenStreetMap app instead of the add form. */
export async function lookAgainNearby() {
	if (app.lookingNearby) return;
	app.lookingNearby = true;
	try {
		const { coords } = await position();
		gpsFix = { lat: coords.latitude, lng: coords.longitude, at: Date.now() };
		const { added, matched } = await api.refreshNearby({ lat: coords.latitude, lng: coords.longitude });
		if (added.length === 0 && matched === 0) {
			toast(null, 'Nothing new within 150 m yet', 'A place just added on OpenStreetMap can take a minute or two to show up. Try again shortly.');
			return;
		}
		if (added.length > 0) {
			const names = added.map((p) => p.name).join(', ');
			toast(null, `Found ${names}`, added.length === 1 ? 'It is on the map for everyone now.' : 'They are on the map for everyone now.');
		}
		// New countries, and any addition of the user's confirmed (announced by the refresh).
		await refresh();
		// The open country's list, with the places just found. Its details stay open.
		const code = app.selected;
		const location = app.location;
		if (code && location) {
			const list = await api.places(location.area.id, code, here());
			if (app.selected === code && app.location === location) app.places = list;
		}
	} catch (error) {
		toast(null, 'Could not look again', message(error));
	} finally {
		app.lookingNearby = false;
	}
}

/** Where the add form's pin starts: the device's position, or the area's centre without one. */
export async function startingPoint(): Promise<{ lat: number; lng: number }> {
	try {
		const { coords } = await position();
		gpsFix = { lat: coords.latitude, lng: coords.longitude, at: Date.now() };
		return { lat: coords.latitude, lng: coords.longitude };
	} catch {
		return here() ?? { lat: app.location?.lat ?? 0, lng: app.location?.lng ?? 0 };
	}
}

/** What an addition brings to its area, as "Ethiopia is new in Leuven". Empty without a bonus. */
function bonusText(bonus: Bonus, code: string, area: string | null, tense: 'is' | 'would be'): string {
	const info = country(code);
	const where = area ? ` in ${area}` : ' here';
	if (bonus === 'continent' && info) {
		const continent = app.catalog?.continents.find((c) => c.code === info.continent)?.name ?? 'Its continent';
		return `${continent} food ${tense} new${where}`;
	}
	if (bonus === 'country' && info) return `${info.name} ${tense} new${where}`;
	return '';
}

/** "Lalibela is on the map. +130 XP, and Ethiopia is new in Leuven!" for each addition
 * OpenStreetMap now has that the user wasn't told about. Runs once at a time. */
let announcing = false;
async function announceConfirmedAdditions() {
	if (announcing) return;
	announcing = true;
	try {
		// A few at a time: any more wait for the next refresh.
		const fresh = (await api.additions()).filter((a) => a.status !== 'pending' && !a.seen).slice(0, 10);
		if (fresh.length === 0) return;
		for (const a of fresh) {
			const flag = country(a.country)?.flag ?? null;
			const icon = flag ? { text: flag } : null;
			if (a.status === 'duplicate') {
				toast(icon, `${a.name} is on the map`, 'Someone added it before you, so it earned no XP. Thanks anyway!');
			} else {
				const bonus = bonusText(a.bonus, a.country, a.area_name, 'is');
				toast(icon, `${a.name} is on the map`, bonus ? `+${a.xp} XP, and ${bonus}!` : `+${a.xp} XP. Thanks for adding it!`);
			}
		}
		await api.markAdditionsSeen(fresh.map((a) => a.id));
	} catch {
		// Shown at the next refresh instead.
	} finally {
		announcing = false;
	}
}

/** Adds a restaurant OpenStreetMap lacks, where the user put the pin. True once added. */
export async function addPlace(fields: AddPlace): Promise<boolean> {
	let place: AddedPlace;
	try {
		place = await api.addPlace(fields);
	} catch (error) {
		toast(null, 'Could not add the restaurant', message(error));
		return false;
	}
	const { xp, bonus, area_name } = place.reward;
	const extra = bonusText(bonus, fields.country.toUpperCase(), area_name, 'would be');
	const reward = `+${xp} XP once it is on OpenStreetMap${extra ? `: ${extra}` : ''}.`;
	toast(null, `${place.name} added`, `You see it right away. ${reward}`);
	// It counts towards its country, for this user, if it lies in the area. A failure here
	// only leaves the counts behind until the next refresh.
	const location = app.location;
	if (location) {
		const discover = await api.discover(location.area.id).catch(() => null);
		if (discover && app.location === location) app.discover = discover;
	}
	await openPlace(place.id);
	return true;
}

/** Updates a place's visits on its card (which shows only the latest) and in its details. */
function setVisits(id: string, update: (visits: Visit[]) => Visit[]) {
	const card = app.places.find((p) => p.id === id);
	if (card) card.visits = update(card.visits).slice(0, 1);
	if (app.detail?.id === id) app.detail.visits = update(app.detail.visits);
}
