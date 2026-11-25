import { beforeEach, describe, expect, it, vi } from 'vitest';
import { deferred } from '../test/fixtures';

type Client = typeof import('./client');

/** The requests the client made: method, path and the token it sent, if any. */
let requests: { method: string; path: string; token: string | null; body: unknown }[];
/** Answers each request; the default says the session is "token-new" and everything else is `{}`. */
let answer: (method: string, path: string, token: string | null) => Response | Promise<Response>;

const json = (body: unknown, status = 200) =>
	new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } });

let client: Client;

beforeEach(async () => {
	requests = [];
	answer = (_method, path) => (path === '/api/session' ? json({ token: 'token-new' }) : json({}));
	vi.stubGlobal(
		'fetch',
		vi.fn(async (path: string, init: RequestInit) => {
			const token = (init.headers as Record<string, string>).authorization?.replace('Bearer ', '') ?? null;
			const method = init.method ?? 'GET';
			requests.push({ method, path, token, body: init.body && JSON.parse(init.body as string) });
			return answer(method, path, token);
		})
	);
	// The client keeps the pending session and the signed-out listener: each test starts afresh.
	vi.resetModules();
	client = await import('./client');
});

describe('raw', () => {
	it('turns an error answer into an ApiError with its message and code', async () => {
		answer = () => json({ error: 'Nothing is open right now.', code: 'nothing_open' }, 404);
		const error = await client.raw('GET', '/api/x', null).catch((e) => e);
		expect(error).toBeInstanceOf(client.ApiError);
		expect(error).toMatchObject({ status: 404, message: 'Nothing is open right now.', code: 'nothing_open' });
	});

	it('says the request failed when the error answer is not JSON', async () => {
		answer = () => new Response('Bad gateway', { status: 502 });
		await expect(client.raw('GET', '/api/x', null)).rejects.toMatchObject({ status: 502, message: 'Request failed (502)' });
	});

	it('says the server cannot be reached when the request does not get through', async () => {
		answer = () => Promise.reject(new TypeError('Failed to fetch'));
		await expect(client.raw('GET', '/api/x', null)).rejects.toMatchObject({ status: 0, message: 'Cannot reach the server.' });
	});

	it('gives nothing for 204', async () => {
		answer = () => new Response(null, { status: 204 });
		await expect(client.raw('DELETE', '/api/x', null)).resolves.toBeUndefined();
	});
});

describe('write', () => {
	it('makes an account the first time, keeps its token and sends it', async () => {
		await client.write('POST', '/api/visits', { place: 'p' });
		expect(requests.map((r) => [r.method, r.path, r.token])).toEqual([
			['POST', '/api/session', null],
			['POST', '/api/visits', 'token-new']
		]);
		expect(requests[1].body).toEqual({ place: 'p' });
		expect(localStorage.getItem(client.TOKEN_KEY)).toBe('token-new');
	});

	it('makes one account for writes that start together', async () => {
		await Promise.all([client.write('POST', '/api/a'), client.write('POST', '/api/b')]);
		expect(requests.filter((r) => r.path === '/api/session')).toHaveLength(1);
	});

	it('keeps a token a restore stored while the account was being made', async () => {
		const session = deferred<Response>();
		answer = (_method, path) => (path === '/api/session' ? session.promise : json({}));
		const writing = client.write('POST', '/api/visits');
		localStorage.setItem(client.TOKEN_KEY, 'token-restored');
		session.resolve(json({ token: 'token-new' }));
		await writing;
		expect(localStorage.getItem(client.TOKEN_KEY)).toBe('token-restored');
		expect(requests.at(-1)?.token).toBe('token-restored');
	});

	it('fails, and says the device was signed out, when the server no longer knows the token', async () => {
		localStorage.setItem(client.TOKEN_KEY, 'token-old');
		const listener = vi.fn();
		client.onSignedOut(listener);
		answer = () => json({ error: 'Unknown token' }, 401);
		await expect(client.write('POST', '/api/visits')).rejects.toMatchObject({ status: 401, code: 'unauthorized' });
		expect(listener).toHaveBeenCalledOnce();
		expect(localStorage.getItem(client.TOKEN_KEY)).toBeNull();
		// No new account is made: that waits for the next write.
		expect(requests.some((r) => r.path === '/api/session')).toBe(false);
	});
});

describe('read', () => {
	it('reads without an account, and does not make one', async () => {
		await client.read('/api/catalog');
		expect(requests).toEqual([expect.objectContaining({ method: 'GET', path: '/api/catalog', token: null })]);
		expect(localStorage.getItem(client.TOKEN_KEY)).toBeNull();
	});

	it('reads again as nobody after a 401, and says the device was signed out', async () => {
		localStorage.setItem(client.TOKEN_KEY, 'token-old');
		const listener = vi.fn();
		client.onSignedOut(listener);
		answer = (_method, _path, token) => (token ? json({ error: 'Unknown token' }, 401) : json({ ok: true }));
		await expect(client.read('/api/discover')).resolves.toEqual({ ok: true });
		expect(requests.map((r) => r.token)).toEqual(['token-old', null]);
		expect(listener).toHaveBeenCalledOnce();
	});

	it('leaves a newer token alone when an older one is refused', async () => {
		localStorage.setItem(client.TOKEN_KEY, 'token-old');
		const listener = vi.fn();
		client.onSignedOut(listener);
		answer = (_method, _path, token) => {
			if (token !== 'token-old') return json({ ok: true });
			// A restore stores its token while this request is on its way.
			localStorage.setItem(client.TOKEN_KEY, 'token-restored');
			return json({ error: 'Unknown token' }, 401);
		};
		await client.read('/api/discover');
		expect(localStorage.getItem(client.TOKEN_KEY)).toBe('token-restored');
		expect(listener).not.toHaveBeenCalled();
		expect(requests.map((r) => r.token)).toEqual(['token-old', 'token-restored']);
	});
});

describe('mine', () => {
	it('gives `none` without asking when the device has no account', async () => {
		await expect(client.mine('/api/account', 'nobody')).resolves.toBe('nobody');
		expect(requests).toEqual([]);
	});
});

describe('writeMine', () => {
	it('does nothing without an account', async () => {
		await client.writeMine('DELETE', '/api/share');
		expect(requests).toEqual([]);
	});
});
