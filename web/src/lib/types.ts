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
export type AddPlace = Schemas['AddPlace'];
/** The place just added, with the XP it earns once OpenStreetMap has it. */
export type AddedPlace = Schemas['AddedPlace'];
export type Bonus = Schemas['Bonus'];
export type Addition = Schemas['Addition'];
/** What reading a place again from OpenStreetMap found. */
export type RefreshResult = Schemas['RefreshResult'];
export type NearbyResult = Schemas['NearbyResult'];
export type Visit = Schemas['Visit'];
export type Day = Schemas['Day'];
export type VisitResult = Schemas['VisitResult'];
export type Health = Schemas['Health'];
export type Account = Schemas['Account'];
export type Device = Schemas['Device'];

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

export type Status = Schemas['Status'];

/** What players call each status: `discovered` is gold, `completed` diamond. */
export const STATUS_LABELS: Record<Status, string> = {
	available: 'To discover',
	discovered: 'Gold',
	platinum: 'Platinum',
	completed: 'Diamond'
};

export const distance = (m: number) => (m < 1000 ? `${Math.round(m / 10) * 10} m` : `${(m / 1000).toFixed(1)} km`);

export const directions = (lat: number, lng: number) =>
	`https://www.openstreetmap.org/directions?route=%3B${lat}%2C${lng}`;
