<script lang="ts">
	import { onMount } from 'svelte';
	import * as api from './api';
	import { accountChanged, toast } from './state.svelte';
	import type { Account, AuthProviders } from './types';

	let { account, onchange }: { account: Account | null; onchange: () => Promise<void> } = $props();

	let providers = $state<AuthProviders | null>(null);
	let email = $state('');
	let code = $state('');
	/** The address a code went to; then the form asks for the code. */
	let sentTo = $state<string | null>(null);
	let busy = $state(false);
	let googleButton = $state<HTMLElement>();

	const names: Record<string, string> = { apple: 'Apple', google: 'Google', email: 'email' };
	const signedIn = $derived(account?.identities ?? []);

	function load(src: string): Promise<void> {
		return new Promise((resolve, reject) => {
			const script = Object.assign(document.createElement('script'), { src, async: true, onload: resolve, onerror: reject });
			document.head.append(script);
		});
	}

	async function act(work: () => Promise<void>, done?: string) {
		busy = true;
		try {
			await work();
			if (done) {
				toast(null, done, 'Your progress now follows you to every device you sign in on.');
				await Promise.all([onchange(), accountChanged()]);
			}
		} catch (e) {
			const message = e instanceof Error ? e.message : 'Something went wrong.';
			if (e instanceof api.ApiError && e.status === 404) toast(null, 'Wrong code', 'Check the code, or ask for a new one.');
			else toast(null, 'That did not work', message);
		} finally {
			busy = false;
		}
	}

	onMount(async () => {
		providers = await api.authProviders().catch(() => null);
	});

	/** Google's script, loaded once, the first time its button is shown. */
	let googleScript: Promise<void> | null = null;

	// Google's own button, as its terms ask; it hands back an ID token. Drawn whenever its
	// slot appears: once the providers are known, and again after signing out.
	$effect(() => {
		const slot = googleButton;
		const clientId = providers?.google;
		if (!slot || !clientId) return;
		googleScript ??= load('https://accounts.google.com/gsi/client');
		void googleScript.then(() => {
			const google = (window as any).google;
			google.accounts.id.initialize({
				client_id: clientId,
				callback: (r: { credential: string }) => act(() => api.signInWith('google', r.credential), 'Signed in with Google')
			});
			google.accounts.id.renderButton(slot, { theme: 'outline', size: 'large', text: 'continue_with', shape: 'pill', locale: 'en', width: Math.min(400, slot.clientWidth) });
		});
	});

	const apple = () =>
		act(async () => {
			const AppleID = (window as any).AppleID ?? (await load('https://appleid.cdn-apple.com/appleauth/static/jsapi/appleid/1/en_US/appleid.auth.js'), (window as any).AppleID);
			AppleID.auth.init({ clientId: providers!.apple, scope: 'email', redirectURI: location.origin + '/', usePopup: true });
			const result = await AppleID.auth.signIn();
			await api.signInWith('apple', result.authorization.id_token);
		}, 'Signed in with Apple');

	const sendCode = () =>
		act(async () => {
			await api.sendEmailCode(email);
			sentTo = email.trim();
			code = '';
		});

	const useCode = () =>
		act(async () => {
			await api.signInWithEmail(sentTo!, code);
			sentTo = null;
		}, 'Signed in');

	const signOut = () =>
		act(async () => {
			if (!confirm('Sign out of this device? Your progress stays in your account; this device starts afresh.')) return;
			await api.signOut();
			toast(null, 'Signed out', 'Sign in again to get your progress back here.');
			await Promise.all([onchange(), accountChanged()]);
		});
</script>

<h3>Account</h3>
{#if signedIn.length}
	<p class="note">
		Signed in with {signedIn.map((i) => (i.email ? `${names[i.provider] ?? i.provider} (${i.email})` : names[i.provider] ?? i.provider)).join(', ')}.
		Your progress follows you to every device you sign in on.
	</p>
	<button class="secondary" disabled={busy} onclick={signOut}>Sign out of this device</button>
{:else}
	<p class="note">Optional. Sign in to keep your progress on all your devices; without it, it stays on this one.</p>
	{#if providers?.google || providers?.apple}
		<div class="providers">
			{#if providers?.google}<div class="google" bind:this={googleButton}></div>{/if}
			{#if providers?.apple}
				<button class="apple" disabled={busy} onclick={apple}>
					<svg viewBox="0 0 17 20" width="14" height="16" aria-hidden="true"><path fill="currentColor" d="M14.2 10.6c0-2.6 2.1-3.8 2.2-3.9-1.2-1.8-3.1-2-3.7-2-1.6-.2-3.1.9-3.9.9-.8 0-2-.9-3.3-.9-1.7 0-3.3 1-4.2 2.5-1.8 3.1-.5 7.7 1.3 10.2.9 1.2 1.9 2.6 3.2 2.6 1.3-.1 1.8-.8 3.3-.8 1.6 0 2 .8 3.3.8 1.4 0 2.3-1.3 3.1-2.5 1-1.4 1.4-2.8 1.4-2.9 0 0-2.7-1-2.7-4zM11.7 3c.7-.8 1.2-2 1-3.1-1 0-2.2.7-2.9 1.5-.6.7-1.2 1.9-1 3 1.1.1 2.2-.6 2.9-1.4z" /></svg>
					Sign in with Apple
				</button>
			{/if}
		</div>
		<p class="or note">or</p>
	{/if}
	{#if sentTo}
		<form
			onsubmit={(e) => {
				e.preventDefault();
				if (code.trim()) void useCode();
			}}
		>
			<label for="sign-in-code">Enter the code we sent to {sentTo}</label>
			<div class="row">
				<input id="sign-in-code" class="code-input" bind:value={code} inputmode="numeric" autocomplete="one-time-code" placeholder="123456" maxlength="7" />
				<button class="action" disabled={busy || !code.trim()}>Sign in</button>
			</div>
			<button type="button" class="link" onclick={() => (sentTo = null)}>Use another address</button>
		</form>
	{:else}
		<form
			onsubmit={(e) => {
				e.preventDefault();
				if (email.trim()) void sendCode();
			}}
		>
			<label for="sign-in-email">Get a sign-in code by email</label>
			<div class="row">
				<input id="sign-in-email" type="email" bind:value={email} autocomplete="email" placeholder="you@example.com" />
				<button class="action" disabled={busy || !email.trim()}>Send code</button>
			</div>
		</form>
	{/if}
{/if}

<style>
	/* Both providers' buttons full width and the same height, Google's own and Apple's. */
	.providers {
		display: grid;
		gap: 8px;
		width: 100%;
	}

	.google {
		min-height: 40px;
	}

	.apple {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 8px;
		height: 40px;
		border-radius: 999px;
		background: #000;
		color: #fff;
		font-weight: 600;
		font-size: var(--text-sm);
	}

	.apple:disabled {
		opacity: 0.6;
	}

	/* A thin line either side of "or". */
	.or {
		display: flex;
		align-items: center;
		gap: 10px;
		width: 100%;
	}

	.or::before,
	.or::after {
		content: '';
		flex: 1;
		border-top: 1px solid var(--line);
	}

	.link {
		justify-self: start;
		color: var(--accent-text);
		font-size: var(--text-xs);
		font-weight: 700;
		padding: 0;
	}
</style>
