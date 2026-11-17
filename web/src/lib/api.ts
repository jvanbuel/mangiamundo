import type { Area, Catalog, Celebration, Discover, Health, Located, Place, Profile } from './types';

const BASE = import.meta.env.VITE_API_BASE ?? '';
const TOKEN_KEY = 'mangiamundo.token';

export class ApiError extends Error {
	constructor(
		public status: number,
		message: string
	) {
		super(message);
	}
}

async function raw<T>(method: string, path: string, token: string | null, body?: unknown): Promise<T> {
	const headers: Record<string, string> = {};
	if (token) headers.authorization = `Bearer ${token}`;
	if (body !== undefined) headers['content-type'] = 'application/json';
	let response: Response;
	try {
		response = await fetch(BASE + path, {
			method,
			headers,
			body: body === undefined ? undefined : JSON.stringify(body)
		});
	} catch {
		throw new ApiError(0, 'Cannot reach the server.');
	}
	if (!response.ok) {
		const detail = await response.json().catch(() => null);
		throw new ApiError(response.status, detail?.error ?? `Request failed (${response.status})`);
	}
	return response.status === 204 ? (undefined as T) : response.json();
}

let pendingToken: Promise<string> | null = null;

/** The anonymous account of this device, created on first use. */
function token(): Promise<string> {
	const stored = localStorage.getItem(TOKEN_KEY);
	if (stored) return Promise.resolve(stored);
	pendingToken ??= raw<{ token: string }>('POST', '/api/session', null)
		.then(({ token }) => {
			localStorage.setItem(TOKEN_KEY, token);
			return token;
		})
		.finally(() => (pendingToken = null));
	return pendingToken;
}

async function authed<T>(method: string, path: string, body?: unknown): Promise<T> {
	try {
		return await raw<T>(method, path, await token(), body);
	} catch (error) {
		// The server no longer knows this token (e.g. its database was reset): start over once.
		if (error instanceof ApiError && error.status === 401) {
			localStorage.removeItem(TOKEN_KEY);
			return raw<T>(method, path, await token(), body);
		}
		throw error;
	}
}

const area = (id: string) => `area=${encodeURIComponent(id)}`;

/** The device's time zone, so event days begin and end at local midnight. */
const tz = () => `tz=${encodeURIComponent(Intl.DateTimeFormat().resolvedOptions().timeZone ?? '')}`;

export const health = () => raw<Health>('GET', '/api/health', null);

export const catalog = () => raw<Catalog>('GET', '/api/countries', null);

export const discover = (areaId: string) => authed<Discover>('GET', `/api/discover?${area(areaId)}&${tz()}`);

/** Distances are measured from `from` when given, otherwise from the area's centre. */
export const places = (areaId: string, country: string, from: { lat: number; lng: number } | null) =>
	authed<Place[]>(
		'GET',
		`/api/places?${area(areaId)}&country=${country}${from ? `&lat=${from.lat}&lng=${from.lng}` : ''}`
	);

export const checkIn = (placeId: string, here: { lat: number; lng: number } | null) =>
	authed<{ visited_at: number; verified: boolean; profile: Profile }>(
		'PUT',
		`/api/visits/${encodeURIComponent(placeId)}?${tz()}`,
		here ?? {}
	);

export const undoCheckIn = (placeId: string) =>
	authed<void>('DELETE', `/api/visits/${encodeURIComponent(placeId)}`);

/** Cities and postal codes matching what someone typed. */
export const searchAreas = (query: string) => raw<Area[]>('GET', `/api/areas?q=${encodeURIComponent(query)}`, null);

/** Suggestions while someone types, nearest first when `near` is known. */
export const suggestAreas = (query: string, near: { lat: number; lng: number } | null) =>
	raw<Area[]>(
		'GET',
		`/api/areas/suggest?q=${encodeURIComponent(query)}${near ? `&lat=${near.lat}&lng=${near.lng}` : ''}`,
		null
	);

/** The city and postal code a point lies in. */
export const locateArea = (lat: number, lng: number) =>
	raw<Located>('GET', `/api/areas/locate?lat=${lat}&lng=${lng}`, null);

export const celebrations = (areaId: string, days = 60) =>
	raw<Celebration[]>('GET', `/api/celebrations?${area(areaId)}&days=${days}&${tz()}`, null);

export const pushKey = () => raw<{ public_key: string }>('GET', '/api/push/key', null);

export interface PushTarget {
	/** Area id. */
	area: string;
	timezone: string;
}

export const savePushSubscription = (subscription: PushSubscriptionJSON, target: PushTarget) =>
	authed<void>('PUT', '/api/push/subscription', { ...subscription, ...target });

export const deletePushSubscription = (endpoint: string) =>
	authed<void>('DELETE', '/api/push/subscription', { endpoint });
