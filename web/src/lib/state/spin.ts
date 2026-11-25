import { prefersReducedMotion } from 'svelte/motion';
import * as api from '../api';
import { loadPlaces } from '../state.svelte';
import { app, message, toast } from './app.svelte';
import { loadPlace, openPlace } from './details';
import { here } from './location';

const NOT_TODAY_KEY = 'mangiamundo.notToday';
/** How long the globe turns before landing on a spin's country. */
export const SPIN_MS = 1600;

/** Places suggested since the area was picked, so spinning again finds another. */
let spunIds: string[] = [];

export function forgetSpins() {
	spunIds = [];
}

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
