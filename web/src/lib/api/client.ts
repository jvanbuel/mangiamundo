const BASE = import.meta.env.VITE_API_BASE ?? '';
export const TOKEN_KEY = 'mangiamundo.token';
/** How this device shows up in the account's device list, such as "Firefox on Windows". */
export function deviceName(): string {
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

export async function raw<T>(method: string, path: string, token: string | null, body?: unknown): Promise<T> {
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

/** The anonymous account of this device, created the first time it has something to keep. */
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

let signedOutListener: (() => void) | null = null;

/** Calls `listener` when the server turns out not to know this device's token any more. */
export function onSignedOut(listener: () => void) {
	signedOutListener = listener;
}

/**
 * The server answered 401 to `sent`: the device was signed out (from another device, or
 * its account was deleted). The token goes and the app is told, so it can say so. No new
 * account is made here: that waits for the next write. A newer token, such as one a
 * restore stored meanwhile, stays.
 */
function signedOut(sent: string) {
	if (localStorage.getItem(TOKEN_KEY) !== sent) return;
	localStorage.removeItem(TOKEN_KEY);
	signedOutListener?.();
}

const unauthorized = (error: unknown) => error instanceof ApiError && error.status === 401;

/**
 * Reads something the server also shows to nobody in particular: with this device's
 * token when it has one, without an account otherwise. Reading never makes an account.
 */
export async function read<T>(path: string): Promise<T> {
	const sent = localStorage.getItem(TOKEN_KEY);
	try {
		return await raw<T>('GET', path, sent);
	} catch (error) {
		if (!sent || !unauthorized(error)) throw error;
		signedOut(sent);
		// With the token a restore stored meanwhile, or as nobody.
		return raw<T>('GET', path, localStorage.getItem(TOKEN_KEY));
	}
}

/** Reads something of the account's: `none` when this device has no account (any more). */
export async function mine<T>(path: string, none: T): Promise<T> {
	const sent = localStorage.getItem(TOKEN_KEY);
	if (!sent) return none;
	try {
		return await raw<T>('GET', path, sent);
	} catch (error) {
		if (!unauthorized(error)) throw error;
		signedOut(sent);
		const newer = localStorage.getItem(TOKEN_KEY);
		return newer ? raw<T>('GET', path, newer) : none;
	}
}

/**
 * Changes something, as this device's account, which is made now when there is none yet.
 * A device that was signed out is told so and the change fails: doing it again starts a
 * new account, knowingly.
 */
export async function write<T>(method: string, path: string, body?: unknown): Promise<T> {
	const sent = await token();
	try {
		return await raw<T>(method, path, sent, body);
	} catch (error) {
		if (!unauthorized(error)) throw error;
		signedOut(sent);
		const newer = localStorage.getItem(TOKEN_KEY);
		if (newer) return raw<T>(method, path, newer, body);
		throw new ApiError(401, 'This device was signed out. Sign in again, or repeat this to start afresh here.', 'unauthorized');
	}
}

/** Like `write`, for a change that only matters to an account that exists: nothing happens without one. */
export async function writeMine(method: string, path: string, body?: unknown): Promise<void> {
	const sent = localStorage.getItem(TOKEN_KEY);
	if (!sent) return;
	try {
		await raw<void>(method, path, sent, body);
	} catch (error) {
		if (!unauthorized(error)) throw error;
		signedOut(sent);
	}
}

export const area = (id: string) => `area=${encodeURIComponent(id)}`;

/** The device's time zone, so event days begin and end at local midnight. */
export const tz = () => `tz=${encodeURIComponent(Intl.DateTimeFormat().resolvedOptions().timeZone ?? '')}`;
