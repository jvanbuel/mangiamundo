import { beforeEach, describe, expect, it, vi } from 'vitest';
import { area, deferred, location, place } from '../test/fixtures';
import type { Suggestion } from '../types';

vi.mock('../api', { spy: true });

let api: typeof import('../api');
let app: typeof import('./app.svelte').app;
let spin: typeof import('./spin');

beforeEach(async () => {
	vi.resetModules();
	api = await import('../api');
	app = (await import('./app.svelte')).app;
	spin = await import('./spin');
	app.location = location();
	vi.mocked(api.places).mockResolvedValue([]);
	vi.mocked(api.place).mockImplementation(async (id) => place({ id }));
});

const suggestion = (id: string, country = 'ET'): Suggestion => ({ country, place: place({ id, countries: [country] }) });
/** The places each spin asked the server to leave out. */
const excluded = () => vi.mocked(api.suggestion).mock.calls.map(([, , exclude]) => exclude);

describe('spin', () => {
	it('lands on a country and opens a place there that is open now', async () => {
		vi.mocked(api.suggestion).mockResolvedValue(suggestion('osm-node-9', 'JP'));

		await spin.spin();

		expect(api.suggestion).toHaveBeenCalledWith('city-leuven', null, [], true);
		expect(app.spun).toBe('osm-node-9');
		expect(app.selected).toBe('JP');
		expect(app.placeId).toBe('osm-node-9');
		expect(app.detail?.id).toBe('osm-node-9');
		expect(window.location.search).toBe('?place=osm-node-9');
		expect(app.spinning).toBe(false);
	});

	it('suggests a place for another time when nothing is open', async () => {
		vi.mocked(api.suggestion)
			.mockRejectedValueOnce(new api.ApiError(404, 'Nothing open', 'nothing_open'))
			.mockResolvedValueOnce(suggestion('osm-node-9'));

		await spin.spin();

		expect(vi.mocked(api.suggestion).mock.calls.map(([, , , open]) => open)).toEqual([true, false]);
		expect(app.spun).toBe('osm-node-9');
		expect(app.toasts.map((t) => t.title)).toEqual(['Nothing open right now']);
	});

	it('finds another place each spin, and starts over once there is none left', async () => {
		vi.mocked(api.suggestion)
			.mockResolvedValueOnce(suggestion('a'))
			.mockResolvedValueOnce(suggestion('b'))
			.mockRejectedValueOnce(new api.ApiError(404, 'Nothing left', 'nothing_left'))
			.mockResolvedValueOnce(suggestion('a'));

		for (let i = 0; i < 4; i++) await spin.spin();

		expect(excluded()).toEqual([[], ['a'], ['a', 'b'], []]);
		expect(app.toasts.map((t) => t.title)).toEqual(['That was every place']);
	});

	it('says when every place in the area has been visited', async () => {
		vi.mocked(api.suggestion).mockRejectedValue(new api.ApiError(404, 'Nothing left', 'nothing_left'));
		await spin.spin();
		expect(app.toasts).toEqual([expect.objectContaining({ title: 'Nowhere new left', text: 'You have been to every place in Leuven.' })]);
	});

	it('replaces the open suggestion, so Back still leaves the details', async () => {
		vi.mocked(api.suggestion).mockResolvedValueOnce(suggestion('a')).mockResolvedValueOnce(suggestion('b'));
		await spin.spin();
		const entries = history.length;
		await spin.spin();
		expect(history.length).toBe(entries);
		expect(window.location.search).toBe('?place=b');
		expect(app.detail?.id).toBe('b');
	});

	it('forgets the spin once the user has moved to another area', async () => {
		const answer = deferred<Suggestion>();
		vi.mocked(api.suggestion).mockReturnValue(answer.promise);

		const spinning = spin.spin();
		app.location = location({ area: area({ id: 'city-gent' }) });
		answer.resolve(suggestion('a'));
		await spinning;

		expect(app.spun).toBeNull();
		expect(app.placeId).toBeNull();
		expect(app.spinning).toBe(false);
	});

	it('does not spin while it is spinning', async () => {
		const answer = deferred<Suggestion>();
		vi.mocked(api.suggestion).mockReturnValue(answer.promise);
		const first = spin.spin();
		await spin.spin();
		answer.resolve(suggestion('a'));
		await first;
		expect(api.suggestion).toHaveBeenCalledOnce();
	});
});

describe('notTodaySpin', () => {
	it('keeps the place out of spins for 20 hours', async () => {
		vi.useFakeTimers({ toFake: ['Date'] });
		try {
			vi.mocked(api.suggestion).mockResolvedValue(suggestion('b'));
			spin.notTodaySpin('a');
			await vi.waitFor(() => expect(app.detail?.id).toBe('b'));
			// Not after the area changes, which forgets the spins.
			spin.forgetSpins();
			vi.setSystemTime(Date.now() + 19 * 3600_000);
			await spin.spin();
			spin.forgetSpins();
			vi.setSystemTime(Date.now() + 2 * 3600_000);
			await spin.spin();
			expect(excluded()).toEqual([['a'], ['a'], []]);
		} finally {
			vi.useRealTimers();
		}
	});
});
