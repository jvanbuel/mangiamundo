// Answers as the server gives them, for the tests. Each takes the fields that matter to a test.
import type { components } from '../api-types';
import type { Area, Catalog, Discover, Location, Place, Visit } from '../types';

type Progress = components['schemas']['Progress'];

/** Progress the tests only tell apart by `marker`: the state code passes it on unread. */
export function progress(marker: string): Progress {
	return { marker } as unknown as Progress;
}

/** What discover shows for an area, told apart by `marker`. */
export function discover(marker: string, where = area()): Discover {
	return { ...progress(marker), area: where } as Discover;
}

/** A catalog with only the countries' codes and flags. */
export function catalog(flags: Record<string, string>): Catalog {
	return { countries: Object.entries(flags).map(([code, flag]) => ({ code, flag })), continents: [] } as unknown as Catalog;
}

export function area(fields: Partial<Area> = {}): Area {
	return { id: 'city-leuven', kind: 'city', name: 'Leuven', detail: null, lat: 50.88, lng: 4.7, ...fields };
}

export function location(fields: Partial<Location> = {}): Location {
	const where = fields.area ?? area();
	return { area: where, lat: where.lat, lng: where.lng, source: 'search', options: [], ...fields };
}

export function place(fields: Partial<Place> = {}): Place {
	return {
		id: 'osm-node-1',
		name: 'Lalibela',
		countries: ['ET'],
		cuisine: 'ethiopian',
		address: null,
		distance_m: 250,
		hours: null,
		lat: 50.88,
		lng: 4.7,
		osm_url: null,
		url: null,
		visits: [],
		...fields
	};
}

/** A visit `daysAgo` days before `now`. */
export function visit(id: number, daysAgo = 0, now = new Date()): Visit {
	return { id, at: Math.floor(now.getTime() / 1000) - daysAgo * 86_400, verified: false };
}

/** A promise with its resolve at hand, to answer a request at a moment of the test's choosing. */
export function deferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (error: unknown) => void;
	const promise = new Promise<T>((res, rej) => {
		resolve = res;
		reject = rej;
	});
	return { promise, resolve, reject };
}
