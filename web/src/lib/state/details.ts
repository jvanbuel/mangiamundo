import { tick } from 'svelte';
import * as api from '../api';
import { app, message, toast } from './app.svelte';
import { here } from './location';

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

export async function loadPlace(id: string) {
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

/** Back and Forward open and close the details. */
export function followHistory() {
	addEventListener('popstate', () => {
		closing = false;
		const id = new URLSearchParams(location.search).get('place');
		if (id) void loadPlace(id);
		else clearPlace();
	});
}
