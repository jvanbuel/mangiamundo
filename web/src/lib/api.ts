import type { Catalog, Discover, GeoResult, Place, Profile } from './types';

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

const area = (lat: number, lng: number, radiusKm: number) =>
	`lat=${lat}&lng=${lng}&radius_km=${radiusKm}`;

export const catalog = () => raw<Catalog>('GET', '/api/countries', null);

export const discover = (lat: number, lng: number, radiusKm: number) =>
	authed<Discover>('GET', `/api/discover?${area(lat, lng, radiusKm)}`);

export const places = (lat: number, lng: number, radiusKm: number, country: string) =>
	authed<Place[]>('GET', `/api/places?${area(lat, lng, radiusKm)}&country=${country}`);

export const checkIn = (placeId: string, here: { lat: number; lng: number } | null) =>
	authed<{ verified: boolean; profile: Profile }>(
		'PUT',
		`/api/visits/${encodeURIComponent(placeId)}`,
		here ?? {}
	);

export const undoCheckIn = (placeId: string) =>
	authed<void>('DELETE', `/api/visits/${encodeURIComponent(placeId)}`);

export const geocode = (query: string) =>
	raw<GeoResult[]>('GET', `/api/geocode?q=${encodeURIComponent(query)}`, null);

export const reverseGeocode = (lat: number, lng: number) =>
	raw<{ name: string | null }>('GET', `/api/geocode/reverse?lat=${lat}&lng=${lng}`, null);
