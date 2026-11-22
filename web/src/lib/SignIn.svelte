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
		if (providers?.google && googleButton) {
			// Google's own button, as its terms ask; it hands back an ID token.
			await load('https://accounts.google.com/gsi/client');
			const google = (window as any).google;
			google.accounts.id.initialize({
				client_id: providers.google,
				callback: (r: { credential: string }) => act(() => api.signInWith('google', r.credential), 'Signed in with Google')
			});
			google.accounts.id.renderButton(googleButton, { theme: 'outline', size: 'large', text: 'continue_with', shape: 'pill' });
		}
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

<h3>Sign in</h3>
{#if signedIn.length}
	<p class="muted small">
		Signed in with {signedIn.map((i) => (i.email ? `${names[i.provider] ?? i.provider} (${i.email})` : names[i.provider] ?? i.provider)).join(', ')}.
	</p>
	<button class="action" disabled={busy} onclick={signOut}>Sign out of this device</button>
{:else}
	<p class="muted small">Optional. Sign in to keep your progress on all your devices; without it, it stays on this one.</p>
	<div class="providers">
		{#if providers?.google}<div bind:this={googleButton}></div>{/if}
		{#if providers?.apple}<button class="apple" disabled={busy} onclick={apple}> Sign in with Apple</button>{/if}
	</div>
	{#if sentTo}
		<form
			onsubmit={(e) => {
				e.preventDefault();
				if (code.trim()) void useCode();
			}}
		>
			<label for="sign-in-code">Enter the code we sent to {sentTo}</label>
			<div class="row">
				<input id="sign-in-code" bind:value={code} inputmode="numeric" autocomplete="one-time-code" placeholder="123456" maxlength="7" />
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
			<label for="sign-in-email">Or get a code by email</label>
			<div class="row">
				<input id="sign-in-email" type="email" bind:value={email} autocomplete="email" placeholder="you@example.com" />
				<button class="action" disabled={busy || !email.trim()}>Send code</button>
			</div>
		</form>
	{/if}
{/if}

<style>
	.providers {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
		margin: 8px 0;
	}

	.providers:empty {
		display: none;
	}

	.apple {
		padding: 9px 16px;
		border-radius: 999px;
		background: #000;
		color: #fff;
		font-weight: 700;
		font-size: var(--text-sm);
	}

	.link {
		color: var(--accent-text);
		font-size: var(--text-xs);
		padding: 4px 0;
	}
</style>
