import type { Catalog, Celebration, CheckInResult, Discover, GeoResult, Place, Profile } from './types';

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

/** A new check-in; returning to a place counts again once a cooldown has passed. */
export const checkIn = (placeId: string, here: { lat: number; lng: number } | null) =>
	authed<CheckInResult>('POST', `/api/visits/${encodeURIComponent(placeId)}`, here ?? {});

/** Logs (or with null, clears) the signature dish eaten at a check-in. */
export const setDish = (checkinId: number, dish: string | null) =>
	authed<Profile>('PATCH', `/api/checkins/${checkinId}`, { dish });

export const undoCheckIn = (checkinId: number) => authed<void>('DELETE', `/api/checkins/${checkinId}`);

export const recoveryCode = () => authed<{ code: string }>('GET', '/api/session/recovery');

export const rotateRecoveryCode = () => authed<{ code: string }>('POST', '/api/session/recovery');

/**
 * Moves this device onto the passport behind a recovery code. Whatever this device
 * had collected is merged in by the server. Returns how many check-ins came along.
 */
export async function restore(code: string): Promise<number> {
	const current = localStorage.getItem(TOKEN_KEY);
	const result = await raw<{ token: string; merged: number }>('POST', '/api/session/restore', current, { code });
	localStorage.setItem(TOKEN_KEY, result.token);
	return result.merged;
}

/** The share card as a PNG, drawn by the server. */
export async function shareCard(
	lat: number,
	lng: number,
	radiusKm: number,
	place: string | null,
	theme: 'light' | 'dark'
): Promise<Blob> {
	const params = new URLSearchParams({ lat: String(lat), lng: String(lng), radius_km: String(radiusKm), theme });
	if (place) params.set('place', place);
	const response = await fetch(`${BASE}/api/share/card.png?${params}`, {
		headers: { authorization: `Bearer ${await token()}` }
	}).catch(() => {
		throw new ApiError(0, 'Cannot reach the server.');
	});
	if (!response.ok) throw new ApiError(response.status, `Could not draw the card (${response.status})`);
	return response.blob();
}

export const geocode = (query: string) =>
	raw<GeoResult[]>('GET', `/api/geocode?q=${encodeURIComponent(query)}`, null);

export const reverseGeocode = (lat: number, lng: number) =>
	raw<{ name: string | null }>('GET', `/api/geocode/reverse?lat=${lat}&lng=${lng}`, null);

export const celebrations = (lat: number, lng: number, radiusKm: number, days = 60) =>
	raw<Celebration[]>('GET', `/api/celebrations?${area(lat, lng, radiusKm)}&days=${days}`, null);

export const pushKey = () => raw<{ public_key: string }>('GET', '/api/push/key', null);

export interface PushTarget {
	lat: number;
	lng: number;
	radius_km: number;
	timezone: string;
}

export const savePushSubscription = (subscription: PushSubscriptionJSON, target: PushTarget) =>
	authed<void>('PUT', '/api/push/subscription', { ...subscription, ...target });

export const deletePushSubscription = (endpoint: string) =>
	authed<void>('DELETE', '/api/push/subscription', { endpoint });
