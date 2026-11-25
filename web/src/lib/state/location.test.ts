import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { area, location } from '../test/fixtures';

vi.mock('../api', { spy: true });

let app: typeof import('./app.svelte').app;
let where: typeof import('./location');

beforeEach(async () => {
	vi.resetModules();
	app = (await import('./app.svelte')).app;
	where = await import('./location');
});

const areas = (n: number) => Array.from({ length: n }, (_, i) => area({ id: `city-${i}`, name: `City ${i}` }));
const ids = () => app.recents.map((a) => a.id);

describe('setLocation', () => {
	it('keeps the areas used lately, newest first, each once', async () => {
		const [a, b] = areas(2);
		await where.setLocation(location({ area: a }));
		await where.setLocation(location({ area: b }));
		await where.setLocation(location({ area: a }));
		expect(ids()).toEqual(['city-0', 'city-1']);
		expect(JSON.parse(localStorage.getItem(where.RECENTS_KEY)!).map((a: { id: string }) => a.id)).toEqual(ids());
	});

	it('keeps at most six', async () => {
		for (const a of areas(8)) await where.setLocation(location({ area: a }));
		expect(ids()).toEqual(['city-7', 'city-6', 'city-5', 'city-4', 'city-3', 'city-2']);
	});

	it('keeps the area being left, also when it was saved before there were recents', async () => {
		const [a, b] = areas(2);
		app.location = location({ area: a });
		await where.setLocation(location({ area: b }));
		expect(ids()).toEqual(['city-1', 'city-0']);
	});

	it('saves the location, and clears what was shown for the area left', async () => {
		app.selected = 'JP';
		app.celebrations = [{} as (typeof app.celebrations)[number]];
		const next = location({ area: area({ id: 'city-gent' }) });
		await where.setLocation(next);
		expect(app.selected).toBeNull();
		expect(app.places).toEqual([]);
		expect(app.celebrations).toEqual([]);
		expect(JSON.parse(localStorage.getItem(where.LOCATION_KEY)!)).toEqual(next);
	});
});

describe('forgetRecent', () => {
	it('removes one area from the recent ones', async () => {
		for (const a of areas(3)) await where.setLocation(location({ area: a }));
		where.forgetRecent('city-1');
		expect(ids()).toEqual(['city-2', 'city-0']);
	});
});

describe('here', () => {
	beforeEach(() => vi.useFakeTimers({ toFake: ['Date'] }));
	afterEach(() => vi.useRealTimers());

	it('is the last GPS position for ten minutes', () => {
		app.location = location();
		where.rememberFix({ latitude: 50.1, longitude: 4.2 } as GeolocationCoordinates);
		expect(where.here()).toEqual({ lat: 50.1, lng: 4.2 });
		expect(where.recentFix()).toMatchObject({ lat: 50.1, lng: 4.2 });
		vi.setSystemTime(Date.now() + 10 * 60_000 + 1);
		expect(where.recentFix()).toBeNull();
		expect(where.here()).toBeNull();
	});

	it('is the location found by GPS, but not one searched for', () => {
		app.location = location({ source: 'gps', lat: 51, lng: 3 });
		expect(where.here()).toEqual({ lat: 51, lng: 3 });
		app.location = location({ source: 'search' });
		expect(where.here()).toBeNull();
	});
});
