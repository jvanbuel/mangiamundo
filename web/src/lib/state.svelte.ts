import * as api from './api';
import * as push from './push';
import { hideSplash } from './splash';
import {
	type AddPlace,
	type AddedPlace,
	type Bonus,
	type Area,
	type Location,
	type Place,
	type UnknownPlace,
	type Unlocked,
	type Visit,
	visitedToday
} from './types';
import { app, country, load, message, toast, type Theme, type ToastIcon } from './state/app.svelte';
import { PROJECTION_KEY, THEME_KEY } from './state/preferences';
import { checkReminders, pushTarget } from './state/reminders';
import { LOCATION_KEY, RECENTS_KEY, here, locate, locationAllowed, position, recentFix, rememberFix } from './state/location';
import { closePlace, followHistory, loadPlace, openPlace } from './state/details';

export { app, country, toast, type Theme, type Toast, type ToastIcon } from './state/app.svelte';
export { setProjection, setTheme } from './state/preferences';
export { toggleReminders } from './state/reminders';
export { chooseArea, forgetRecent, locate, setLocation, startingPoint, switchArea } from './state/location';
export { closePlace, openPlace } from './state/details';
export { SPIN_MS, notTodaySpin, spin } from './state/spin';

let requestId = 0;
/** A ?country= link that arrived before there was anything to discover: opened on the first load. */
let pendingCountry: string | null = null;

/** Asks for the server's version until it answers once. */
function loadServer() {
	if (app.server && app.server !== 'not reached') return;
	api
		.health()
		.then((h) => (app.server = [h.version ?? 'unknown', h.commit].filter(Boolean).join(' ')))
		.catch(() => (app.server = 'not reached'));
}

/**
 * The server no longer knows this device: it was signed out from another device, or its
 * account is gone. Says so, and shows everything again as for someone without an account,
 * so that nothing of the old account stays on screen as if it were still here. A new
 * account only starts with the next check-in or other change.
 */
function signedOut() {
	toast(null, 'You were signed out', 'This device no longer has your progress. Sign in or use a recovery code to get it back.');
	// Whatever shows the account reads it again.
	app.publicLink++;
	void accountChanged();
}

export async function init() {
	api.onSignedOut(signedOut);
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
	followHistory();
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

export async function loadPlaces(code: string | null) {
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
function announce(unlocked: Unlocked[]) {
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
	// Nothing of the answer is shown once the device is in another account.
	const sameAccount = () => accountGeneration === generation;
	// Progress is only shown when it is also still about this area.
	const stillHere = () => app.location?.area.id === location.area.id && sameAccount();
	try {
		if (!undoing) {
			const fresh = recentFix();
			const result = await api.checkIn(place.id, location.area.id, fresh && { lat: fresh.lat, lng: fresh.lng });
			if (!sameAccount()) return;
			setVisits(place.id, (visits) => [result.visit, ...visits.filter((v) => v.id !== result.visit.id)]);
			// Details still loading may have read the visits from before this change.
			if (app.placeId === place.id && !app.detail) void loadPlace(place.id);
			if (result.progress && stillHere()) app.discover = { ...result.progress, area: before.area };
			announce(result.unlocked);
		} else {
			const result = await api.removeVisit(place.visits[0].id, location.area.id).catch(async (error) => {
				if (!(error instanceof api.ApiError && error.code === 'not_found')) throw error;
				// Gone already, from another device or by a retry whose answer was lost: ask what is left.
				const [rest, progress] = await Promise.all([api.place(place.id, null), api.discover(location.area.id)]);
				return { visits: rest.visits, profile: progress.profile, progress };
			});
			if (!sameAccount()) return;
			// The server's list of the visits left: the card only knew the latest.
			setVisits(place.id, () => result.visits);
			if (app.placeId === place.id && !app.detail) void loadPlace(place.id);
			if (result.progress && stillHere()) app.discover = { ...result.progress, area: before.area };
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
		const location = app.location;
		const generation = accountGeneration;
		const result = await api.reportClosed(place.id, location?.area.id ?? null);
		if (accountGeneration !== generation) return;
		app.places = app.places.filter((p) => p.id !== place.id);
		if (app.placeId === place.id) closePlace();
		toast(null, 'Thanks for letting us know', `${place.name} is hidden for you now.`);
		// The place no longer counts towards its countries.
		const shown = app.discover;
		if (result.progress && shown && shown.area.id === location?.area.id && app.location?.area.id === shown.area.id) {
			app.discover = { ...result.progress, area: shown.area };
		}
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
		rememberFix(coords);
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
