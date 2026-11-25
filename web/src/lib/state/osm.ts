import * as api from '../api';
import type { Place } from '../types';
import { app, message, toast } from './app.svelte';
import { closePlace, loadPlace } from './details';
import { refresh } from './discover';
import { here, position, rememberFix } from './location';

/** The place whose OpenStreetMap page the user opened to edit it, read again when they come back. */
let editing: string | null = null;
/** A refresh waiting for the one running to finish. */
let queuedRefresh: string | null = null;

export function editingOnOsm(id: string) {
	editing = id;
}

/** Back from editing a place on OpenStreetMap: show the edit now, not after the next import. */
export function followOsmEdits() {
	document.addEventListener('visibilitychange', () => {
		if (document.visibilityState !== 'visible' || !editing) return;
		const id = editing;
		editing = null;
		// A refresh still running may predate the edit: this one goes after it.
		if (app.refreshingPlace) queuedRefresh = id;
		else void refreshPlace(id, true);
	});
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
