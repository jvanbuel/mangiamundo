import type { Account, AuthProviders } from '../types';
import { ApiError, TOKEN_KEY, deviceName, mine, raw, write, writeMine } from './client';

/** Whether this device has an account yet; it gets one with its first check-in or other change. */
export const hasAccount = () => localStorage.getItem(TOKEN_KEY) !== null;

/** What a device without an account shows: nothing is on the server until its first change. */
const NO_ACCOUNT: Account = { created_at: 0, recovery_code_created_at: null, devices: [], public_profile: null, identities: [] };

/**
 * The account's recovery code status and the devices signed in to it. A device without an
 * account has an empty one, with no devices.
 */
export const account = () => mine<Account>('/api/account', NO_ACCOUNT);

/** A new recovery code, shown once. The previous one stops working. */
export const newRecoveryCode = () => write<{ code: string }>('POST', '/api/account/recovery-code');

/** A code that signs another device in to this account, once, within 10 minutes. */
export const newLinkCode = () => write<{ code: string; expires_at: number }>('POST', '/api/account/link-code');

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

/** Signs this device out of its account; it starts afresh, without one. */
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
export const removeDevice = (id: string) => writeMine('DELETE', `/api/account/devices/${encodeURIComponent(id)}`);

/** Deletes the account and everything in it. This device starts over without one. */
export async function deleteAccount(): Promise<void> {
	await writeMine('DELETE', '/api/account');
	localStorage.removeItem(TOKEN_KEY);
}
