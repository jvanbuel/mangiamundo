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

export interface EventProgress {
	id: string;
	title: string;
	description: string;
	countries: string[];
	/** First and last day, inclusive, as YYYY-MM-DD. */
	starts: string;
	ends: string;
	target: number;
	progress: number;
	earned: boolean;
	status: 'upcoming' | 'active' | 'ended';
	places_nearby: number;
}

export interface Celebration {
	id: string;
	title: string;
	description: string;
	countries: string[];
	starts: string;
	ends: string;
	approximate: boolean;
	/** The celebration's countries that have places in the area. */
	nearby: { code: string; places: number }[];
}

export type AreaKind = 'city' | 'postal_code';

/** A city (every postal code in it) or a single postal code. */
export interface Area {
	/** Opaque, e.g. "osm-relation-196311". */
	id: string;
	/** "Leuven", or "3000 Leuven" for a postal code. */
	name: string;
	kind: AreaKind;
	/** Where it is, to tell same-named areas apart: "Flemish Brabant, Belgium". */
	detail: string | null;
	lat: number;
	lng: number;
}

/** The areas a point lies in. */
export interface Located {
	city: Area | null;
	postal_code: Area | null;
}

export interface Discover {
	area: Area & { fetched_at: number };
	countries: CountryProgress[];
	continents: GroupProgress[];
	world: GroupProgress;
	places: number;
	places_visited: number;
	badges: Badge[];
	profile: Profile;
	events: EventProgress[];
}

export interface Place {
	id: string;
	name: string;
	lat: number;
	lng: number;
	address: string | null;
	url: string | null;
	cuisine: string | null;
	/** Raw OSM opening_hours. */
	opening_hours: string | null;
	/** Null when the hours are unknown. */
	open_now: boolean | null;
	/** Short status line, e.g. "Open until 22:00". */
	hours_text: string | null;
	countries: string[];
	distance_m: number;
	visited_at: number | null;
	verified: boolean;
}

export interface Location {
	area: Area;
	/** Where the map pin goes: the GPS position, or the area's centre. */
	lat: number;
	lng: number;
	source: 'gps' | 'search';
	/** The city and postal code found at the GPS position, to switch between. */
	options: Area[];
}

export type Status = 'available' | 'discovered' | 'completed';

export function statusOf(c: { places: number; visited: number }): Status {
	if (c.visited === 0) return 'available';
	return c.visited >= c.places ? 'completed' : 'discovered';
}

/** From `/api/health`. An older server sends only `ok`. */
export interface Health {
	ok: boolean;
	version?: string;
	commit?: string | null;
}
