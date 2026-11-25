import * as api from '../api';
import type { AddPlace, AddedPlace, Bonus, UnknownPlace } from '../types';
import { app, country, message, toast } from './app.svelte';
import { openPlace } from './details';
import { here } from './location';

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
