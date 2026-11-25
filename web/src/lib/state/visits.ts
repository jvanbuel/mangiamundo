import * as api from '../api';
import { type Place, type Unlocked, type Visit, visitedToday } from '../types';
import { app, country, message, toast, type ToastIcon } from './app.svelte';
import { currentGeneration } from './account';
import { closePlace, loadPlace } from './details';
import { recentFix } from './location';

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
	const generation = currentGeneration();
	// Nothing of the answer is shown once the device is in another account.
	const sameAccount = () => currentGeneration() === generation;
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
		const generation = currentGeneration();
		const result = await api.reportClosed(place.id, location?.area.id ?? null);
		if (currentGeneration() !== generation) return;
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

/** Updates a place's visits on its card (which shows only the latest) and in its details. */
function setVisits(id: string, update: (visits: Visit[]) => Visit[]) {
	const card = app.places.find((p) => p.id === id);
	if (card) card.visits = update(card.visits).slice(0, 1);
	if (app.detail?.id === id) app.detail.visits = update(app.detail.visits);
}
