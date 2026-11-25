import type { ClosedResult, NearbyResult, Place, RefreshResult, Suggestion } from '../types';
import { area, read, tz, write } from './client';

/** Distances are measured from `from` when given, otherwise from the area's centre. */
export const places = (areaId: string, country: string, from: { lat: number; lng: number } | null) =>
	read<Place[]>(`/api/places?${area(areaId)}&country=${country}${from ? `&lat=${from.lat}&lng=${from.lng}` : ''}`
	);

/** A random unvisited place to try, open now unless `open` is false. */
export const suggestion = (areaId: string, from: { lat: number; lng: number } | null, exclude: string[], open: boolean) =>
	read<Suggestion>(`/api/suggestion?${area(areaId)}&open=${open}&exclude=${exclude.map(encodeURIComponent).join(',')}${from ? `&lat=${from.lat}&lng=${from.lng}` : ''}`
	);

/** One place with all of the user's visits; the distance is measured from `from` when given. */
export const place = (placeId: string, from: { lat: number; lng: number } | null) =>
	read<Place>(`/api/places/${encodeURIComponent(placeId)}${from ? `?lat=${from.lat}&lng=${from.lng}` : ''}`
	);

/** "This place has closed": hides it for this user, and for everyone after a second report. */
export const reportClosed = (placeId: string, areaId: string | null) =>
	write<ClosedResult>('PUT', `/api/places/${encodeURIComponent(placeId)}/closed?${areaId ? `${area(areaId)}&` : ''}${tz()}`);

/** Reads a place again from OpenStreetMap, for an edit made there since the daily import. */
export const refreshPlace = (placeId: string, from: { lat: number; lng: number } | null) =>
	write<RefreshResult>(
		'POST',
		`/api/places/${encodeURIComponent(placeId)}/refresh${from ? `?lat=${from.lat}&lng=${from.lng}` : ''}`
	);

/** Asks OpenStreetMap for restaurants within 150 m of `at`, for one just mapped there. */
export const refreshNearby = (at: { lat: number; lng: number }) =>
	write<NearbyResult>('POST', `/api/places/nearby/refresh?lat=${at.lat}&lng=${at.lng}`);
