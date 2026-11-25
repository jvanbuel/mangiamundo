import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { area, catalog, deferred, discover, location, place, progress, visit } from '../test/fixtures';
import type { VisitRemoved, VisitResult } from '../types';

vi.mock('../api', { spy: true });

let api: typeof import('../api');
let app: typeof import('./app.svelte').app;
let visits: typeof import('./visits');
let account: typeof import('./account');
let where: typeof import('./location');

beforeEach(async () => {
	// One fixed moment, at midday: a visit made in the test and one made in the expectation
	// are the same visit, and "today" doesn't depend on when the tests run.
	vi.useFakeTimers({ toFake: ['Date'], now: new Date(2026, 10, 25, 12) });
	// The state modules keep their own counters and the app's state: each test starts afresh.
	vi.resetModules();
	api = await import('../api');
	app = (await import('./app.svelte')).app;
	visits = await import('./visits');
	account = await import('./account');
	where = await import('./location');
	app.catalog = catalog({ ET: '🇪🇹' });
	app.location = location();
	app.discover = discover('before');
});

afterEach(() => vi.useRealTimers());

const checkedIn = (fields: Partial<VisitResult> = {}): VisitResult => ({
	visit: visit(7),
	progress: progress('after'),
	unlocked: [],
	profile: {} as VisitResult['profile'],
	...fields
});

const removed = (fields: Partial<VisitRemoved> = {}): VisitRemoved => ({
	visits: [],
	progress: progress('after'),
	profile: {} as VisitRemoved['profile'],
	...fields
});

describe('toggleVisit, checking in', () => {
	it('shows the visit on the card and in the details, and the new progress', async () => {
		const older = visit(3, 5);
		const shown = place({ visits: [older] });
		app.places = [shown];
		app.placeId = shown.id;
		app.detail = place({ visits: [older] });
		vi.mocked(api.checkIn).mockResolvedValue(checkedIn());

		await visits.toggleVisit(shown);

		expect(api.checkIn).toHaveBeenCalledWith(shown.id, 'city-leuven', null);
		expect(app.places[0].visits).toEqual([visit(7)]);
		expect(app.detail?.visits).toEqual([visit(7), older]);
		expect(app.discover).toEqual(discover('after'));
		expect(app.busyPlace).toBeNull();
	});

	it('checks in again on another day', async () => {
		vi.mocked(api.checkIn).mockResolvedValue(checkedIn());
		await visits.toggleVisit(place({ visits: [visit(3, 1)] }));
		expect(api.checkIn).toHaveBeenCalled();
		expect(api.removeVisit).not.toHaveBeenCalled();
	});

	it('sends a recent GPS position, to verify the visit', async () => {
		where.rememberFix({ latitude: 50.1, longitude: 4.2 } as GeolocationCoordinates);
		vi.mocked(api.checkIn).mockResolvedValue(checkedIn());
		await visits.toggleVisit(place());
		expect(api.checkIn).toHaveBeenCalledWith('osm-node-1', 'city-leuven', { lat: 50.1, lng: 4.2 });
	});

	it('announces what the visit unlocked', async () => {
		vi.mocked(api.checkIn).mockResolvedValue(
			checkedIn({ unlocked: [{ kind: 'country', country: 'ET', badge: null, level: null, title: 'Ethiopia!', text: 'A new country' }] })
		);
		await visits.toggleVisit(place());
		expect(app.toasts).toEqual([expect.objectContaining({ icon: { text: '🇪🇹' }, title: 'Ethiopia!', text: 'A new country' })]);
	});

	it('keeps the visit but not the progress once the user has moved to another area', async () => {
		const answer = deferred<VisitResult>();
		vi.mocked(api.checkIn).mockReturnValue(answer.promise);
		const shown = place();
		app.places = [shown];

		const toggling = visits.toggleVisit(shown);
		app.location = location({ area: area({ id: 'city-gent', name: 'Gent' }) });
		app.discover = discover('gent');
		answer.resolve(checkedIn());
		await toggling;

		expect(app.places[0].visits).toEqual([visit(7)]);
		expect(app.discover).toEqual(discover('gent'));
	});

	it('shows nothing of the answer once the device is in another account', async () => {
		const answer = deferred<VisitResult>();
		vi.mocked(api.checkIn).mockReturnValue(answer.promise);
		const shown = place();
		app.places = [shown];

		const toggling = visits.toggleVisit(shown);
		await account.accountChanged();
		app.places = [shown];
		answer.resolve(checkedIn({ unlocked: [{ kind: 'level', level: 3, country: null, badge: null, title: 'Level 3', text: '' }] }));
		await toggling;

		expect(app.places[0].visits).toEqual([]);
		expect(app.toasts.map((t) => t.title)).not.toContain('Level 3');
	});

	it('runs one check-in at a time', async () => {
		const answer = deferred<VisitResult>();
		vi.mocked(api.checkIn).mockReturnValue(answer.promise);
		const first = visits.toggleVisit(place({ id: 'a' }));
		await visits.toggleVisit(place({ id: 'b' }));
		answer.resolve(checkedIn());
		await first;
		expect(api.checkIn).toHaveBeenCalledOnce();
	});

	it('says so when it fails, and lets the next one through', async () => {
		vi.mocked(api.checkIn).mockRejectedValue(new api.ApiError(500, 'The server broke'));
		await visits.toggleVisit(place());
		expect(app.toasts).toEqual([expect.objectContaining({ title: 'That did not work', text: 'The server broke' })]);
		expect(app.busyPlace).toBeNull();
	});
});

describe('toggleVisit, undoing a visit made today', () => {
	it('removes the latest visit and shows the visits the server has left', async () => {
		const older = visit(3, 5);
		const shown = place({ visits: [visit(7)] });
		app.places = [shown];
		app.placeId = shown.id;
		app.detail = place({ visits: [visit(7), older] });
		vi.mocked(api.removeVisit).mockResolvedValue(removed({ visits: [older] }));

		await visits.toggleVisit(shown);

		expect(api.removeVisit).toHaveBeenCalledWith(7, 'city-leuven');
		expect(api.checkIn).not.toHaveBeenCalled();
		expect(app.places[0].visits).toEqual([older]);
		expect(app.detail?.visits).toEqual([older]);
		expect(app.discover).toEqual(discover('after'));
	});

	it('asks what is left when the visit was already gone', async () => {
		const older = visit(3, 5);
		const shown = place({ visits: [visit(7)] });
		app.places = [shown];
		vi.mocked(api.removeVisit).mockRejectedValue(new api.ApiError(404, 'Not found', 'not_found'));
		vi.mocked(api.place).mockResolvedValue(place({ visits: [older] }));
		vi.mocked(api.discover).mockResolvedValue(discover('server'));

		await visits.toggleVisit(shown);

		expect(app.places[0].visits).toEqual([older]);
		expect(app.discover).toEqual(discover('server'));
		expect(app.toasts).toEqual([]);
	});
});

describe('reportClosed', () => {
	it('hides the place, closes its details and shows the new progress', async () => {
		const shown = place();
		app.places = [shown, place({ id: 'other' })];
		app.placeId = shown.id;
		app.detail = shown;
		vi.mocked(api.reportClosed).mockResolvedValue({ progress: progress('after') } as Awaited<ReturnType<typeof api.reportClosed>>);

		await visits.reportClosed(shown);

		expect(app.places.map((p) => p.id)).toEqual(['other']);
		expect(app.placeId).toBeNull();
		expect(app.discover).toEqual(discover('after'));
		expect(app.toasts.map((t) => t.title)).toEqual(['Thanks for letting us know']);
	});
});
