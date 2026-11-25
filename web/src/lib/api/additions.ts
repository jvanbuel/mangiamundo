import type { AddPlace, AddedPlace, Addition, UnknownPlace } from '../types';
import { area, mine, read, write, writeMine } from './client';

/** Adds a restaurant OpenStreetMap lacks. Only this user sees it until OSM has it. */
export const addPlace = (place: AddPlace) => write<AddedPlace>('POST', '/api/places', place);

/** Restaurants in the area OpenStreetMap gives no cuisine, nearest to `from` first. */
export const unknownPlaces = (areaId: string, from: { lat: number; lng: number } | null) =>
	read<UnknownPlace[]>(`/api/unknown-places?${area(areaId)}${from ? `&lat=${from.lat}&lng=${from.lng}` : ''}`);

/** "It serves food from …": like adding a place, only this user sees it until OSM has the cuisine. */
export const suggestCuisine = (placeId: string, country: string) =>
	write<AddedPlace>('POST', `/api/unknown-places/${encodeURIComponent(placeId)}/cuisine`, { country });

/** "It serves no particular country's food": hides it for this user, and for everyone after two. */
export const noCuisine = (placeId: string) =>
	write<void>('PUT', `/api/unknown-places/${encodeURIComponent(placeId)}/no-cuisine`);

/** The restaurants this user added, newest first, with their rewards. */
export const additions = () => mine<Addition[]>('/api/additions', []);

/** The user was told about these confirmed additions, by `id` (at most 100). */
export const markAdditionsSeen = (ids: string[]) => writeMine('PUT', '/api/additions/seen', { ids });
