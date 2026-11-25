import { describe, expect, it } from 'vitest';
import { place, visit } from './test/fixtures';
import { directions, distance, visitedToday } from './types';

describe('visitedToday', () => {
	const now = new Date(2026, 10, 25, 12);

	it('is false without visits', () => {
		expect(visitedToday(place(), now)).toBe(false);
	});

	it('looks only at the latest visit', () => {
		expect(visitedToday(place({ visits: [visit(2, 0, now), visit(1, 3, now)] }), now)).toBe(true);
		expect(visitedToday(place({ visits: [visit(1, 1, now)] }), now)).toBe(false);
	});

	it('goes by the local day, not the last 24 hours', () => {
		const lateLastNight = new Date(2026, 10, 24, 23, 30);
		const earlyThisMorning = new Date(2026, 10, 25, 0, 30);
		const at = (d: Date) => Math.floor(d.getTime() / 1000);
		expect(visitedToday(place({ visits: [{ id: 1, at: at(lateLastNight), verified: true }] }), now)).toBe(false);
		expect(visitedToday(place({ visits: [{ id: 1, at: at(earlyThisMorning), verified: true }] }), now)).toBe(true);
	});
});

describe('distance', () => {
	it('rounds metres to tens below a kilometre', () => {
		expect(distance(0)).toBe('0 m');
		expect(distance(244)).toBe('240 m');
		expect(distance(995)).toBe('1000 m');
	});

	it('gives kilometres with one decimal from a kilometre on', () => {
		expect(distance(1000)).toBe('1.0 km');
		expect(distance(12_345)).toBe('12.3 km');
	});
});

describe('directions', () => {
	it('routes to the place on OpenStreetMap', () => {
		expect(directions(50.88, 4.7)).toBe('https://www.openstreetmap.org/directions?route=%3B50.88%2C4.7');
	});
});
