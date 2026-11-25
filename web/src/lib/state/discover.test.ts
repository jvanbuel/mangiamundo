import { beforeEach, describe, expect, it, vi } from 'vitest';
import { deferred, discover as shown, location, place } from '../test/fixtures';
import type { Discover, Place } from '../types';

vi.mock('../api', { spy: true });

let api: typeof import('../api');
let app: typeof import('./app.svelte').app;
let discover: typeof import('./discover');

beforeEach(async () => {
	vi.resetModules();
	api = await import('../api');
	app = (await import('./app.svelte')).app;
	discover = await import('./discover');
	app.location = location();
	vi.mocked(api.celebrations).mockResolvedValue([]);
	vi.mocked(api.additions).mockResolvedValue([]);
});

describe('refresh', () => {
	it('shows the area\'s countries', async () => {
		vi.mocked(api.discover).mockResolvedValue(shown('leuven'));
		await discover.refresh();
		expect(api.discover).toHaveBeenCalledWith('city-leuven');
		expect(app.discover).toEqual(shown('leuven'));
		expect(app.loading).toBe(false);
		expect(app.error).toBeNull();
	});

	it('shows only the latest refresh', async () => {
		const first = deferred<Discover>();
		vi.mocked(api.discover).mockReturnValueOnce(first.promise).mockResolvedValueOnce(shown('second'));
		const refreshing = discover.refresh();
		await discover.refresh();
		first.resolve(shown('first'));
		await refreshing;
		expect(app.discover).toEqual(shown('second'));
		expect(app.loading).toBe(false);
	});

	it('says what went wrong', async () => {
		vi.mocked(api.discover).mockRejectedValue(new api.ApiError(503, 'OpenStreetMap is busy'));
		app.discover = shown('old');
		await discover.refresh();
		expect(app.error).toBe('OpenStreetMap is busy');
		expect(app.discover).toBeNull();
	});

	it('opens the country a link asked for once there is something to discover', async () => {
		vi.mocked(api.discover).mockResolvedValue({ ...shown('leuven'), countries: [{ code: 'JP' }] } as Discover);
		vi.mocked(api.places).mockResolvedValue([place()]);
		discover.openCountryLater('JP');
		await discover.refresh();
		await vi.waitFor(() => expect(app.places).toHaveLength(1));
		expect(app.selected).toBe('JP');
	});
});

describe('loadPlaces', () => {
	it('shows only the list asked for last', async () => {
		const japan = deferred<Place[]>();
		vi.mocked(api.places).mockReturnValueOnce(japan.promise).mockResolvedValueOnce([place({ id: 'et' })]);
		const loading = discover.loadPlaces('JP');
		await discover.loadPlaces('ET');
		japan.resolve([place({ id: 'jp' })]);
		await loading;
		expect(app.selected).toBe('ET');
		expect(app.places.map((p) => p.id)).toEqual(['et']);
		expect(app.placesLoading).toBe(false);
	});

	it('clears the list when no country is selected', async () => {
		app.places = [place()];
		await discover.loadPlaces(null);
		expect(app.places).toEqual([]);
		expect(api.places).not.toHaveBeenCalled();
	});
});
