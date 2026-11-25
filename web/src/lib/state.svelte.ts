import * as api from './api';
import { hideSplash } from './splash';
import type { Area, Location } from './types';
import { app, load, message, type Theme } from './state/app.svelte';
import { PROJECTION_KEY, THEME_KEY } from './state/preferences';
import { checkReminders } from './state/reminders';
import { LOCATION_KEY, RECENTS_KEY, locate, locationAllowed } from './state/location';
import { followHistory, loadPlace } from './state/details';
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
export { addPlace, noCuisine, suggestCuisine, unknownPlaces } from './state/additions';

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
