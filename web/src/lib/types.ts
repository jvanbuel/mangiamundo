export interface Country {
	code: string;
	numeric: string | null;
	name: string;
	continent: string;
	flag: string;
	lat: number;
	lng: number;
}

export interface Catalog {
	countries: Country[];
	continents: { code: string; name: string }[];
}

export interface CountryProgress {
	code: string;
	continent: string;
	places: number;
	visited: number;
}

export interface GroupProgress {
	code: string;
	name: string;
	available: number;
	discovered: number;
	completed: number;
}

export type BadgeKind = 'visits' | 'countries' | 'completed' | 'continent' | 'world' | 'everything';

export interface Badge {
	id: string;
	kind: BadgeKind;
	title: string;
	description: string;
	progress: number;
	target: number;
	earned: boolean;
}

export interface Profile {
	xp: number;
	level: number;
	level_xp: number;
	next_level_xp: number;
	visits: number;
	countries: number;
}

export interface Discover {
	area: { lat: number; lng: number; radius_m: number; provider: string; fetched_at: number };
	countries: CountryProgress[];
	continents: GroupProgress[];
	world: GroupProgress;
	places: number;
	places_visited: number;
	badges: Badge[];
	profile: Profile;
}

export interface Place {
	id: string;
	name: string;
	lat: number;
	lng: number;
	address: string | null;
	url: string | null;
	cuisine: string | null;
	countries: string[];
	distance_m: number;
	visited_at: number | null;
	verified: boolean;
}

export interface GeoResult {
	name: string;
	lat: number;
	lng: number;
}

export interface Location {
	lat: number;
	lng: number;
	name: string | null;
	source: 'gps' | 'search';
}

export type Status = 'available' | 'discovered' | 'completed';

export function statusOf(c: { places: number; visited: number }): Status {
	if (c.visited === 0) return 'available';
	return c.visited >= c.places ? 'completed' : 'discovered';
}
