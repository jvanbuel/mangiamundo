export interface Country {
	code: string;
	numeric: string | null;
	name: string;
	continent: string;
	flag: string;
	lat: number;
	lng: number;
}

/** A signature dish of a country; logging one at a check-in earns bonus XP. */
export interface Dish {
	id: string;
	country: string;
	name: string;
}

export interface Catalog {
	countries: Country[];
	continents: { code: string; name: string }[];
	dishes: Dish[];
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

export type BadgeKind =
	| 'visits'
	| 'countries'
	| 'completed'
	| 'continent'
	| 'world'
	| 'everything'
	| 'dishes'
	| 'regular';

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
	dishes: number;
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

export interface Discover {
	area: { lat: number; lng: number; radius_m: number; provider: string; fetched_at: number };
	countries: CountryProgress[];
	continents: GroupProgress[];
	world: GroupProgress;
	places: number;
	places_visited: number;
	badges: Badge[];
	profile: Profile;
	events: EventProgress[];
	/** Ids of every signature dish the user logged. */
	dishes_tried: string[];
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
	/** First check-in; set means the place counts as visited. */
	visited_at: number | null;
	verified: boolean;
	/** Number of check-ins here. */
	visits: number;
	last_visited_at: number | null;
	last_checkin_id: number | null;
	last_dish: string | null;
	/** When checking in again becomes possible, if not right now. */
	next_check_in_at: number | null;
}

export interface CheckInResult {
	place_id: string;
	checkin_id: number;
	verified: boolean;
	profile: Profile;
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
