import * as api from './api';
import { hideSplash } from './splash';
import {
	type AddPlace,
	type AddedPlace,
	type Bonus,
	type Area,
	type Location,
	type Place,
	type UnknownPlace
} from './types';
import { app, country, load, message, toast, type Theme } from './state/app.svelte';
import { PROJECTION_KEY, THEME_KEY } from './state/preferences';
import { checkReminders, pushTarget } from './state/reminders';
import { LOCATION_KEY, RECENTS_KEY, here, locate, locationAllowed, position, rememberFix } from './state/location';
import { closePlace, followHistory, loadPlace, openPlace } from './state/details';
import { loadServer, openCountryLater, refresh, select } from './state/discover';
import { loadAccount, signedOut } from './state/account';

export { app, country, toast, type Theme, type Toast, type ToastIcon } from './state/app.svelte';
export { setProjection, setTheme } from './state/preferences';
export { toggleReminders } from './state/reminders';
export { chooseArea, forgetRecent, locate, setLocation, startingPoint, switchArea } from './state/location';
export { closePlace, openPlace } from './state/details';
export { SPIN_MS, notTodaySpin, spin } from './state/spin';
export { refresh, select } from './state/discover';
export { accountChanged, loadAccount } from './state/account';
export { reportClosed, toggleVisit } from './state/visits';

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
	if (code && !app.discover) openCountryLater(code);
	else if (code && app.discover?.countries.some((c) => c.code === code)) await select(code);
	const id = params.get('place');
	history.replaceState(null, '', id ? `?place=${encodeURIComponent(id)}` : location.pathname);
	if (id) await loadPlace(id);
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
export async function announceConfirmedAdditions() {
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
