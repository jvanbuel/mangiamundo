import * as api from '../api';
import * as push from '../push';
import { hideSplash } from '../splash';
import type { Location, Place } from '../types';
import { app, message, toast } from './app.svelte';
import { announceConfirmedAdditions } from './additions';
import { closePlace } from './details';
import { here } from './location';
import { pushTarget } from './reminders';

let requestId = 0;
/** A ?country= link that arrived before there was anything to discover: opened on the first load. */
let pendingCountry: string | null = null;

/** Opens a country's list once the first discover is in. */
export function openCountryLater(code: string) {
	pendingCountry = code;
}

/** Asks for the server's version until it answers once. */
export function loadServer() {
	if (app.server && app.server !== 'not reached') return;
	api
		.health()
		.then((h) => (app.server = [h.version ?? 'unknown', h.commit].filter(Boolean).join(' ')))
		.catch(() => (app.server = 'not reached'));
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
		const result = await placesOf(code, location);
		if (list === lists) app.places = result;
	} catch (error) {
		if (list === lists) toast(null, 'Could not load places', message(error));
	} finally {
		if (list === lists) app.placesLoading = false;
	}
}

/** A country the area has no place of, once the area's countries are known. Its list is
 * then the nearest places elsewhere. */
export function elsewhere(code: string): boolean {
	return !!app.discover && !app.discover.countries.some((c) => c.code === code);
}

/** A country's list: its places in the area, or the nearest elsewhere when there are none,
 * measured from the user when known, otherwise from the map pin. */
export function placesOf(code: string, location: Location): Promise<Place[]> {
	if (!elsewhere(code)) return api.places(location.area.id, code, here());
	return api.nearest(code, here() ?? { lat: location.lat, lng: location.lng });
}
