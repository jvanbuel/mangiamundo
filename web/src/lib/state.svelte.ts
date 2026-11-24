import { tick } from 'svelte';
import { prefersReducedMotion } from 'svelte/motion';
import * as api from './api';
import * as push from './push';
import { hideSplash } from './splash';
import {
	type Account,
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
	type UnknownPlace,
	type Unlocked,
	type Visit,
	visitedToday
} from './types';

const LOCATION_KEY = 'mangiamundo.location';
const PROJECTION_KEY = 'mangiamundo.projection';
const THEME_KEY = 'mangiamundo.theme';
const RECENTS_KEY = 'mangiamundo.recentAreas';
const NOT_TODAY_KEY = 'mangiamundo.notToday';
/** How long the globe turns before landing on a spin's country. */
export const SPIN_MS = 1600;
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
	/** While true the map turns the globe; when it ends the map flies to the selected country. */
	spinning: false,
	/** The place the last spin landed on, whose details offer to spin again. */
	spun: null as string | null,
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
	/** This device's account, for showing whether and how it is signed in. */
	account: null as Account | null,
	/** Goes up whenever the public link is made or turned off, for whatever shows it. */
	publicLink: 0,
	/** Areas used lately, newest first, for switching back quickly. */
	recents: [] as Area[]
});

/** The last real GPS position, used to verify check-ins even when browsing another city. */
let gpsFix: { lat: number; lng: number; at: number } | null = null;
let requestId = 0;
/** A ?country= link that arrived before there was anything to discover: opened on the first load. */
let pendingCountry: string | null = null;
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
	void loadAccount();
	const stored = load<Location>(LOCATION_KEY);
	if (stored?.area) {
		app.location = stored;
		await refresh();
		void hideSplash();
		// Keep following the device when the last location came from GPS.
		if (stored.source === 'gps') void locate(true);
	} else {
		// First visit: ask for the location only once the welcome card has explained why,
		// when the user presses its button, unless the browser already allows it.
		void hideSplash();
		if (await locationAllowed()) await locate(true);
	}
	await followLink();
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
	if (code && !app.discover) pendingCountry = code;
	else if (code && app.discover?.countries.some((c) => c.code === code)) await select(code);
	const id = params.get('place');
	history.replaceState(null, '', id ? `?place=${encodeURIComponent(id)}` : location.pathname);
	if (id) await loadPlace(id);
}

/** Opens a place's details, as a new history entry so Back closes them. */
export async function openPlace(id: string) {
	history.pushState({ place: id }, '', `?place=${encodeURIComponent(id)}`);
	await loadPlace(id);
}

/** Places suggested since the area was picked, so spinning again finds another. */
let spunIds: string[] = [];

/** Places put off with "Not today", by id, with when they come back (ms). */
function notToday(): Record<string, number> {
	try {
		const all: Record<string, number> = JSON.parse(localStorage.getItem(NOT_TODAY_KEY) ?? '{}');
		return Object.fromEntries(Object.entries(all).filter(([, until]) => until > Date.now()));
	} catch {
		return {};
	}
}

/** Spins the globe: it lands on a country and opens an unvisited place there, open now if any is. */
export async function spin() {
	const location = app.location;
	if (!location || app.spinning) return;
	app.spinning = true;
	// The globe turns for a while even when the answer comes back at once.
	const turning = new Promise((done) => setTimeout(done, prefersReducedMotion.current ? 0 : SPIN_MS));
	const exclude = [...spunIds, ...Object.keys(notToday())];
	try {
		let found;
		try {
			found = await api.suggestion(location.area.id, here(), exclude, true);
		} catch (error) {
			if (!(error instanceof api.ApiError && error.code === 'nothing_open')) throw error;
			found = await api.suggestion(location.area.id, here(), exclude, false);
			toast(null, 'Nothing open right now', 'Here is one for another time.');
		}
		await turning;
		if (app.location !== location) return;
		spunIds.push(found.place.id);
		app.spun = found.place.id;
		void loadPlaces(found.country);
		// Landing: the map stops turning and flies to the country just selected.
		app.spinning = false;
		// Spinning again from a suggestion replaces it, so Back still leaves the details.
		if (app.placeId) {
			history.replaceState({ place: found.place.id }, '', `?place=${encodeURIComponent(found.place.id)}`);
			await loadPlace(found.place.id);
		} else await openPlace(found.place.id);
	} catch (error) {
		await turning;
		if (error instanceof api.ApiError && error.code === 'nothing_left') {
			if (spunIds.length) {
				spunIds = [];
				toast(null, 'That was every place', 'Spin again to start over.');
			} else toast(null, 'Nowhere new left', `You have been to every place in ${location.area.name}.`);
		} else toast(null, 'Could not spin', message(error));
	} finally {
		app.spinning = false;
	}
}

/** "Not today": keeps the place out of spins for 20 hours, and spins again. */
export function notTodaySpin(id: string) {
	try {
		localStorage.setItem(NOT_TODAY_KEY, JSON.stringify({ ...notToday(), [id]: Date.now() + 20 * 3600_000 }));
	} catch {
		// Storage unavailable: it still stays out of spins until the page closes.
	}
	void spin();
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
	spunIds = [];
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

/** Whether the browser will share the location without asking. */
async function locationAllowed(): Promise<boolean> {
	try {
		return (await navigator.permissions.query({ name: 'geolocation' })).state === 'granted';
	} catch {
		return false;
	}
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
		if (id === requestId) {
			app.discover = result;
			const code = pendingCountry;
			pendingCountry = null;
			if (code && result.countries.some((c) => c.code === code)) void select(code);
		}
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
	await loadPlaces(code);
}

/** Goes up with every change of account, so an answer for the previous one can be told apart. */
let accountGeneration = 0;

/** After this device changes account: the open list and details show the new account's visits. */
export async function accountChanged() {
	accountGeneration++;
	await Promise.all([refresh(), loadPlaces(app.selected), app.placeId && loadPlace(app.placeId), loadAccount()]);
}

/** Loads the account; a failure only means the header can't say whether it's signed in. */
export async function loadAccount() {
	try {
		app.account = await api.account();
	} catch {
		app.account = null;
	}
}

/** Counts list loads, so only the latest one shows its result. */
let lists = 0;

async function loadPlaces(code: string | null) {
	const list = ++lists;
	app.selected = code;
	app.places = [];
	const location = app.location;
	if (!code || !location) return;
	app.placesLoading = true;
	try {
		const result = await api.places(location.area.id, code, here());
		if (list === lists) app.places = result;
	} catch (error) {
		if (list === lists) toast(null, 'Could not load places', message(error));
	} finally {
		if (list === lists) app.placesLoading = false;
	}
}

/** Announces what the server says a check-in unlocked. */
function celebrate(unlocked: Unlocked[]) {
	for (const u of unlocked) {
		const icon: ToastIcon =
			u.kind === 'country' ? { text: country(u.country ?? '')?.flag ?? '' }
			: u.kind === 'badge' && u.badge ? { badge: u.badge }
			: u.kind === 'event' ? { badge: 'event' }
			: u.kind === 'level' ? { text: String(u.level ?? '') }
			: null;
		toast(icon, u.title, u.text);
	}
}

export async function toggleVisit(place: Place) {
	const before = app.discover;
	const location = app.location;
	if (!before || !location || app.busyPlace) return;
	// Visited today: undo that visit. Otherwise check in, also when there are older visits.
	const undoing = visitedToday(place);
	app.busyPlace = place.id;
	const generation = accountGeneration;
	// Progress is only shown when it is still about this area and this account.
	const stillHere = () => app.location?.area.id === location.area.id && accountGeneration === generation;
	try {
		if (!undoing) {
			const fresh = gpsFix && Date.now() - gpsFix.at < 10 * 60_000 ? gpsFix : null;
			const result = await api.checkIn(place.id, location.area.id, fresh && { lat: fresh.lat, lng: fresh.lng });
			setVisits(place.id, (visits) => [result.visit, ...visits.filter((v) => v.id !== result.visit.id)]);
			// Details still loading may have read the visits from before this change.
			if (app.placeId === place.id && !app.detail) void loadPlace(place.id);
			if (result.progress && stillHere()) app.discover = { ...result.progress, area: before.area };
			celebrate(result.unlocked);
		} else {
			const removed = place.visits[0].id;
			await api.removeVisit(removed);
			setVisits(place.id, (visits) => visits.filter((v) => v.id !== removed));
			if (app.placeId === place.id && !app.detail) void loadPlace(place.id);
			// The server has the remaining visits; details shown here may be out of date.
			const rest = await api.place(place.id, null).then((p) => p.visits, () => null);
			if (rest) setVisits(place.id, () => rest);
			else if (app.detail?.id === place.id) {
				// The card shows only the latest visit: fall back on the older ones in the details.
				const card = app.places.find((p) => p.id === place.id);
				if (card) card.visits = app.detail.visits.slice(0, 1);
			}
			const after = await api.discover(location.area.id);
			if (stillHere()) app.discover = after;
		}
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
	await added(place, fields.country);
	return true;
}

/** Restaurants in the area with unknown food, nearest to the user first. Empty on failure. */
export async function unknownPlaces(): Promise<UnknownPlace[]> {
	const location = app.location;
	if (!location) return [];
	return api.unknownPlaces(location.area.id, here()).catch(() => []);
}

/** "It serves food from …" for a restaurant with unknown food. True once saved. */
export async function suggestCuisine(placeId: string, code: string): Promise<boolean> {
	let place: AddedPlace;
	try {
		place = await api.suggestCuisine(placeId, code);
	} catch (error) {
		toast(null, 'Could not save that', message(error));
		return false;
	}
	await added(place, code);
	return true;
}

/** "It serves no particular country's food". True once saved. */
export async function noCuisine(placeId: string): Promise<boolean> {
	try {
		await api.noCuisine(placeId);
		return true;
	} catch (error) {
		toast(null, 'Could not save that', message(error));
		return false;
	}
}

/** Tells what a new pending place earns, counts it in and opens it. */
async function added(place: AddedPlace, code: string) {
	const { xp, bonus, area_name } = place.reward;
	const extra = bonusText(bonus, code.toUpperCase(), area_name, 'would be');
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
}

/** Updates a place's visits on its card (which shows only the latest) and in its details. */
function setVisits(id: string, update: (visits: Visit[]) => Visit[]) {
	const card = app.places.find((p) => p.id === id);
	if (card) card.visits = update(card.visits).slice(0, 1);
	if (app.detail?.id === id) app.detail.visits = update(app.detail.visits);
}
