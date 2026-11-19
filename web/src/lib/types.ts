import type { components } from './api-types';

/** Response types, generated from `contract/openapi.json` by `npm run api-types`. */
type Schemas = components['schemas'];

export type Country = Schemas['Country'];
export type Catalog = Schemas['Catalog'];
export type CountryProgress = Schemas['CountryProgress'];
export type GroupProgress = Schemas['GroupProgress'];
export type BadgeKind = Schemas['BadgeKind'];
export type Badge = Schemas['Badge'];
export type Profile = Schemas['Profile'];
export type EventProgress = Schemas['EventProgress'];
export type Celebration = Schemas['CelebrationView'];
export type AreaKind = Schemas['AreaKind'];
/** A city (every postal code in it) or a single postal code. */
export type Area = Schemas['AreaInfo'];
export type AreaProgress = Schemas['LoadProgress'];
export type Located = Schemas['Located'];
export type Discover = Schemas['Discover'];
export type Place = Schemas['PlaceView'];
export type Visit = Schemas['Visit'];
export type Day = Schemas['Day'];
export type VisitResult = Schemas['VisitResult'];
export type Health = Schemas['Health'];

export interface Location {
	area: Area;
	/** Where the map pin goes: the GPS position, or the area's centre. */
	lat: number;
	lng: number;
	source: 'gps' | 'search';
	/** The city and postal code found at the GPS position, to switch between. */
	options: Area[];
}

/** Whether the latest visit to a place was today, in the device's time zone. */
export function visitedToday(place: Place, now = new Date()): boolean {
	const latest = place.visits[0];
	return latest !== undefined && new Date(latest.at * 1000).toDateString() === now.toDateString();
}

export type Status = 'available' | 'discovered' | 'completed';

export function statusOf(c: { places: number; visited: number }): Status {
	if (c.visited === 0) return 'available';
	return c.visited >= c.places ? 'completed' : 'discovered';
}

export const distance = (m: number) => (m < 1000 ? `${Math.round(m / 10) * 10} m` : `${(m / 1000).toFixed(1)} km`);

export const directions = (lat: number, lng: number) =>
	`https://www.openstreetmap.org/directions?route=%3B${lat}%2C${lng}`;
