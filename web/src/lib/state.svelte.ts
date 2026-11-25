import * as api from './api';
import { hideSplash } from './splash';
import {
	type AddPlace,
	type AddedPlace,
	type Bonus,
	type Area,
	type Location,
	type UnknownPlace
} from './types';
import { app, country, load, message, toast, type Theme } from './state/app.svelte';
import { PROJECTION_KEY, THEME_KEY } from './state/preferences';
import { checkReminders, pushTarget } from './state/reminders';
import { LOCATION_KEY, RECENTS_KEY, here, locate, locationAllowed } from './state/location';
import { followHistory, loadPlace, openPlace } from './state/details';
import { loadServer, openCountryLater, refresh, select } from './state/discover';
import { followOsmEdits } from './state/osm';
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
export { editingOnOsm, lookAgainNearby, refreshPlace } from './state/osm';

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
	followOsmEdits();
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
