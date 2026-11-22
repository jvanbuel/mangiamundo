import type { Account, AuthProviders, AddPlace, AddedPlace, Addition, NearbyResult, RefreshResult, Area, AreaProgress, Catalog, Celebration, Discover, Health, Located, Place, ShareLink, Suggestion, VisitResult } from './types';

const BASE = import.meta.env.VITE_API_BASE ?? '';
const TOKEN_KEY = 'mangiamundo.token';
/** How this device shows up in the account's device list, such as "Firefox on Windows". */
function deviceName(): string {
	const ua = navigator.userAgent;
	const browser = /Edg\//.test(ua) ? 'Edge' : /Firefox\//.test(ua) ? 'Firefox' : /Chrome\//.test(ua) ? 'Chrome' : /Safari\//.test(ua) ? 'Safari' : 'Browser';
	const os = /iPhone|iPad/.test(ua) ? 'iPhone' : /Android/.test(ua) ? 'Android' : /Mac/.test(ua) ? 'Mac' : /Windows/.test(ua) ? 'Windows' : /Linux/.test(ua) ? 'Linux' : null;
	return os ? `${browser} on ${os}` : browser;
}

export class ApiError extends Error {
	constructor(
		public status: number,
		message: string,
		/** The server's `code`, such as `nothing_open`. */
		public code?: string
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
		throw new ApiError(response.status, detail?.error ?? `Request failed (${response.status})`, detail?.code);
	}
	return response.status === 204 ? (undefined as T) : response.json();
}

let pendingToken: Promise<string> | null = null;

/** The anonymous account of this device, created on first use. */
function token(): Promise<string> {
	const stored = localStorage.getItem(TOKEN_KEY);
	if (stored) return Promise.resolve(stored);
	pendingToken ??= raw<{ token: string }>('POST', '/api/session', null, { device: deviceName() })
		.then(({ token }) => {
			// A restore that finished meanwhile stored its token; keep that one.
			const restored = localStorage.getItem(TOKEN_KEY);
			if (restored) return restored;
			localStorage.setItem(TOKEN_KEY, token);
			return token;
		})
		.finally(() => (pendingToken = null));
	return pendingToken;
}

async function authed<T>(method: string, path: string, body?: unknown): Promise<T> {
	const sent = await token();
	try {
		return await raw<T>(method, path, sent, body);
	} catch (error) {
		// The server no longer knows this token (e.g. its database was reset): start over once.
		// A newer token, such as one a restore stored meanwhile, stays.
		if (error instanceof ApiError && error.status === 401) {
			if (localStorage.getItem(TOKEN_KEY) === sent) localStorage.removeItem(TOKEN_KEY);
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

/** A random unvisited place to try, open now unless `open` is false. */
export const suggestion = (areaId: string, from: { lat: number; lng: number } | null, exclude: string[], open: boolean) =>
	authed<Suggestion>(
		'GET',
		`/api/suggestion?${area(areaId)}&open=${open}&exclude=${exclude.map(encodeURIComponent).join(',')}${from ? `&lat=${from.lat}&lng=${from.lng}` : ''}`
	);

/** One place with all of the user's visits; the distance is measured from `from` when given. */
export const place = (placeId: string, from: { lat: number; lng: number } | null) =>
	authed<Place>(
		'GET',
		`/api/places/${encodeURIComponent(placeId)}${from ? `?lat=${from.lat}&lng=${from.lng}` : ''}`
	);

export const checkIn = (placeId: string, here: { lat: number; lng: number } | null) =>
	authed<VisitResult>('POST', `/api/places/${encodeURIComponent(placeId)}/visits?${tz()}`, {
		position: here
	});

export const removeVisit = (visitId: number) => authed<void>('DELETE', `/api/visits/${visitId}`);

/** Makes (or moves to this area) the public page of the user's progress. */
export const shareProgress = (areaId: string) => authed<ShareLink>('POST', `/api/share?${area(areaId)}`);

/** Turns the public page off. */
export const unshareProgress = () => authed<void>('DELETE', '/api/share');

/** A public page for one check-in. */
export const shareVisit = (visitId: number) => authed<ShareLink>('POST', `/api/visits/${visitId}/share`);

/** "This place has closed": hides it for this user, and for everyone after a second report. */
export const reportClosed = (placeId: string) =>
	authed<void>('PUT', `/api/places/${encodeURIComponent(placeId)}/closed`);

/** Adds a restaurant OpenStreetMap lacks. Only this user sees it until OSM has it. */
export const addPlace = (place: AddPlace) => authed<AddedPlace>('POST', '/api/places', place);

/** Reads a place again from OpenStreetMap, for an edit made there since the daily import. */
export const refreshPlace = (placeId: string, from: { lat: number; lng: number } | null) =>
	authed<RefreshResult>(
		'POST',
		`/api/places/${encodeURIComponent(placeId)}/refresh${from ? `?lat=${from.lat}&lng=${from.lng}` : ''}`
	);

/** Asks OpenStreetMap for restaurants within 150 m of `at`, for one just mapped there. */
export const refreshNearby = (at: { lat: number; lng: number }) =>
	authed<NearbyResult>('POST', `/api/places/nearby/refresh?lat=${at.lat}&lng=${at.lng}`);

/** The restaurants this user added, newest first, with their rewards. */
export const additions = () => authed<Addition[]>('GET', '/api/additions');

/** The user was told about these confirmed additions, by `id` (at most 100). */
export const markAdditionsSeen = (ids: string[]) => authed<void>('PUT', '/api/additions/seen', { ids });

/** Cities and postal codes matching what someone typed. */
export const searchAreas = (query: string) => raw<Area[]>('GET', `/api/areas?q=${encodeURIComponent(query)}`, null);

/** Suggestions while someone types, nearest first when `near` is known. */
export const suggestAreas = (query: string, near: { lat: number; lng: number } | null) =>
	raw<Area[]>(
		'GET',
		`/api/areas/suggest?q=${encodeURIComponent(query)}${near ? `&lat=${near.lat}&lng=${near.lng}` : ''}`,
		null
	);

/** How far loading an area has come while discover waits on it; null when it isn't loading. */
export const areaProgress = (areaId: string) =>
	raw<AreaProgress | null>('GET', `/api/areas/${encodeURIComponent(areaId)}/progress`, null);

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
	authed<void>('POST', '/api/push/subscription/delete', { endpoint });

/** Whether this device has an account yet; it gets one on first use. */
export const hasAccount = () => localStorage.getItem(TOKEN_KEY) !== null;

/** The account's recovery code status and the devices signed in to it. */
export const account = () => authed<Account>('GET', '/api/account');

/** A new recovery code, shown once. The previous one stops working. */
export const newRecoveryCode = () => authed<{ code: string }>('POST', '/api/account/recovery-code');

/** A code that signs another device in to this account, once, within 10 minutes. */
export const newLinkCode = () => authed<{ code: string; expires_at: number }>('POST', '/api/account/link-code');

/**
 * Signs this device in to the account of a recovery or link code. What this device did
 * before moves into that account.
 */
/** Signs this device in to the account of a recovery or link code. */
export const restore = (code: string) => switchAccount('/api/session/restore', { code });

/** Which sign-in methods the server has set up. */
export const authProviders = () => raw<AuthProviders>('GET', '/api/auth/providers', null);

/** Emails a 6-digit sign-in code. */
export const sendEmailCode = (email: string) => raw<void>('POST', '/api/auth/email/code', null, { email });

export const signInWithEmail = (email: string, code: string) => switchAccount('/api/auth/email', { email, code });

/** Signs in with an ID token from Google's or Apple's sign-in. */
export const signInWith = (provider: 'google' | 'apple', idToken: string) => switchAccount(`/api/auth/${provider}`, { id_token: idToken });

/** Signs this device out of its account; it starts afresh with a new, empty one. */
export async function signOut(): Promise<void> {
	const me = (await account()).devices.find((d) => d.current);
	if (me) await removeDevice(me.id);
	localStorage.removeItem(TOKEN_KEY);
}

/**
 * Sends the device's token along to an endpoint that answers with another account's
 * session (what this device held merges into it), and keeps that session.
 */
async function switchAccount(path: string, fields: Record<string, string>): Promise<void> {
	const body = { ...fields, device: deviceName() };
	let session: { token: string };
	try {
		session = await raw<{ token: string }>('POST', path, localStorage.getItem(TOKEN_KEY), body);
	} catch (error) {
		// This device's session was ended (say, from another device): restore without it.
		if (!(error instanceof ApiError && error.status === 401)) throw error;
		localStorage.removeItem(TOKEN_KEY);
		session = await raw<{ token: string }>('POST', path, null, body);
	}
	localStorage.setItem(TOKEN_KEY, session.token);
}

/** Signs another device out of this account. */
export const removeDevice = (id: string) => authed<void>('DELETE', `/api/account/devices/${encodeURIComponent(id)}`);

/** Deletes the account and everything in it. This device starts over with a new one. */
export async function deleteAccount(): Promise<void> {
	await authed<void>('DELETE', '/api/account');
	localStorage.removeItem(TOKEN_KEY);
}
