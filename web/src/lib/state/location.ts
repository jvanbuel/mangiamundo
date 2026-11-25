import * as api from '../api';
import type { Area, Located, Location } from '../types';
import { closePlace, forgetSpins, refresh } from '../state.svelte';
import { app, message } from './app.svelte';

export const LOCATION_KEY = 'mangiamundo.location';
export const RECENTS_KEY = 'mangiamundo.recentAreas';
/** Areas kept as recent, including the current one; the picker shows the others. */
const MAX_RECENTS = 6;

/** The last real GPS position, used to verify check-ins even when browsing another city. */
let gpsFix: { lat: number; lng: number; at: number } | null = null;

/** Remembers a real GPS position. */
export function rememberFix(coords: GeolocationCoordinates) {
	gpsFix = { lat: coords.latitude, lng: coords.longitude, at: Date.now() };
}

/** The last real GPS position, while it is recent. */
export function recentFix() {
	return gpsFix && Date.now() - gpsFix.at < 10 * 60_000 ? gpsFix : null;
}

function optionsOf(located: Located): Area[] {
	return [located.city, located.postal_code].filter((a): a is Area => a !== null);
}

export async function setLocation(location: Location) {
	// The area being left counts too, so a location saved before recents existed is kept.
	const left = app.location ? remembering(app.location.area, app.recents) : app.recents;
	app.location = location;
	forgetSpins();
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
export function here(): { lat: number; lng: number } | null {
	if (gpsFix && Date.now() - gpsFix.at < 10 * 60_000) return { lat: gpsFix.lat, lng: gpsFix.lng };
	return app.location?.source === 'gps' ? { lat: app.location.lat, lng: app.location.lng } : null;
}

export function position(): Promise<GeolocationPosition> {
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
export async function locationAllowed(): Promise<boolean> {
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
