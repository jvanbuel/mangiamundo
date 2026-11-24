<script lang="ts">
	import { onMount } from 'svelte';
	import * as api from './api';
	import { accountChanged, app, toast } from './state.svelte.ts';
	import SignIn from './SignIn.svelte';
	import type { Account } from './types';

	let account = $state<Account | null>(null);
	let error = $state<string | null>(null);
	let busy = $state(false);
	/** Shown once, right after it is made. */
	let recoveryCode = $state<string | null>(null);
	let link = $state<{ code: string; expires_at: number } | null>(null);
	let now = $state(Date.now() / 1000);
	let entered = $state('');

	const message = (e: unknown) => (e instanceof Error ? e.message : 'Something went wrong.');
	const linkLeft = $derived(link ? Math.max(0, Math.ceil((link.expires_at - now) / 60)) : 0);

	/** Counts loads, so only the latest one's answer is shown. */
	let loads = 0;

	async function load() {
		const mine = ++loads;
		try {
			const loaded = await api.account();
			if (mine !== loads) return;
			account = loaded;
			app.account = loaded;
			error = null;
		} catch (e) {
			if (mine === loads) error = message(e);
		}
	}

	// On opening, and again when sharing elsewhere on the page makes or moves the link.
	$effect(() => {
		void app.publicLink;
		void load();
	});

	onMount(() => {
		const timer = setInterval(() => {
			now = Date.now() / 1000;
			if (link && link.expires_at <= now) link = null;
		}, 5000);
		return () => clearInterval(timer);
	});

	async function act(work: () => Promise<void>) {
		busy = true;
		try {
			await work();
		} catch (e) {
			toast(null, 'That did not work', message(e));
		} finally {
			busy = false;
		}
	}

	const makeRecoveryCode = () =>
		act(async () => {
			if (account?.recovery_code_created_at && !confirm('Make a new recovery code? The old one stops working.')) return;
			recoveryCode = (await api.newRecoveryCode()).code;
			await load();
		});

	const makeLinkCode = () =>
		act(async () => {
			link = await api.newLinkCode();
			now = Date.now() / 1000;
		});

	const copy = (text: string) =>
		navigator.clipboard?.writeText(text).then(
			() => toast(null, 'Copied', 'Keep it somewhere safe, like a password manager.'),
			() => undefined
		);

	const restore = () =>
		act(async () => {
			try {
				await api.restore(entered);
			} catch (e) {
				if (e instanceof api.ApiError && e.status === 404) {
					toast(null, 'Code not recognised', 'Check it, or make a new one: a link code works once, for 10 minutes.');
					return;
				}
				throw e;
			}
			entered = '';
			recoveryCode = null;
			link = null;
			toast(null, 'Welcome back', 'Your progress is on this device now.');
			await Promise.all([load(), accountChanged()]);
		});

	const signOut = (id: string) =>
		act(async () => {
			await api.removeDevice(id);
			await load();
		});

	const deleteEverything = () =>
		act(async () => {
			if (!confirm('Delete your account? Every check-in, badge and added place is gone for good, on all your devices.')) return;
			await api.deleteAccount();
			recoveryCode = null;
			link = null;
			toast(null, 'Account deleted', 'You start fresh on this device.');
			await Promise.all([load(), accountChanged()]);
		});

	const unshare = () =>
		act(async () => {
			await api.unshareProgress();
			app.publicLink++;
		});

	const ago = (at: number) => {
		const days = Math.floor((now - at) / 86400);
		if (days < 1) return 'today';
		if (days === 1) return 'yesterday';
		return days < 60 ? `${days} days ago` : new Date(at * 1000).toLocaleDateString();
	};
</script>

<section class="account">
	<div class="group"><SignIn {account} onchange={load} /></div>

	<div class="group">
		<h3>{account?.identities.length ? 'Recovery code' : 'Without signing in'}</h3>
		<p class="note">
			{account?.identities.length
				? 'A recovery code also gets your progress back, without signing in.'
				: 'Your progress lives on this device. A recovery code gets it back after clearing your browser or on a new phone.'}
		</p>
		{#if recoveryCode}
			<div class="code" aria-live="polite">
				<code>{recoveryCode}</code>
				<button class="secondary" onclick={() => copy(recoveryCode!)}>Copy</button>
			</div>
			<p class="note">Shown only now. Anyone with this code can use your account.</p>
		{:else}
			<button class="secondary" disabled={busy || !account} onclick={makeRecoveryCode}>
				{account?.recovery_code_created_at ? 'Make a new recovery code' : 'Make a recovery code'}
			</button>
			{#if account?.recovery_code_created_at}
				<p class="note">You made one {ago(account.recovery_code_created_at)}.</p>
			{/if}
		{/if}
	</div>

	<div class="group">
		<h3>Use it on another device</h3>
		{#if link}
			<div class="code" aria-live="polite">
				<code>{link.code}</code>
				<button class="secondary" onclick={() => copy(link!.code)}>Copy</button>
			</div>
			<p class="note">Enter it there under Settings within {linkLeft} {linkLeft === 1 ? 'minute' : 'minutes'}. It works once.</p>
		{:else}
			<p class="note">Show a one-time code here and enter it on the other device.</p>
			<button class="secondary" disabled={busy} onclick={makeLinkCode}>Show a link code</button>
		{/if}

		<form
			onsubmit={(e) => {
				e.preventDefault();
				if (entered.trim()) void restore();
			}}
		>
			<label for="restore-code">Have a recovery or link code?</label>
			<div class="row">
				<input id="restore-code" class="code-input" bind:value={entered} placeholder="XXXX-XXXX" autocomplete="off" autocapitalize="characters" spellcheck="false" />
				<button class="action" disabled={busy || !entered.trim()}>Use</button>
			</div>
			<p class="note">What you did on this device so far is added to that account.</p>
		</form>
	</div>

	{#if error}
		<div class="group"><p class="note">{error}</p></div>
	{:else if account && account.devices.length > 1}
		<div class="group">
			<h3>Signed-in devices</h3>
			<ul>
				{#each account.devices as device (device.id)}
					<li>
						<span>
							<strong>{device.name ?? 'Unnamed device'}</strong>
							<span class="note">{device.current ? 'This device' : `Used ${ago(device.last_used_at)}`}</span>
						</span>
						{#if !device.current}
							<button class="secondary" disabled={busy} onclick={() => signOut(device.id)}>Sign out</button>
						{/if}
					</li>
				{/each}
			</ul>
		</div>
	{/if}

	{#if account?.public_profile}
		<div class="group">
			<h3>Public link</h3>
			<p class="note">
				Anyone with <a href={account.public_profile} target="_blank" rel="noopener">your link</a> can see your map, level and badges.
				Turning it off breaks the link; sharing again makes a new one.
			</p>
			<button class="secondary" disabled={busy} onclick={unshare}>Turn off the public link</button>
		</div>
	{/if}

	<!-- A device that only looked around has no account yet: there is nothing to delete. -->
	{#if account?.devices.length}
		<div class="group">
			<button class="danger" disabled={busy} onclick={deleteEverything}>Delete my account</button>
		</div>
	{/if}
</section>

<style>
	/* One look for every section of the settings, SignIn's included. */
	.account {
		margin-top: 4px;
	}

	.account :global(.group) {
		display: grid;
		gap: 8px;
		justify-items: start;
		padding: 14px 0;
		border-top: 1px solid var(--line);
	}

	.account :global(.group > *) {
		max-width: 100%;
	}

	.account :global(h3) {
		margin: 0;
		font-size: var(--text-sm);
		font-weight: 800;
	}

	.account :global(.note) {
		margin: 0;
		font-size: var(--text-xs);
		line-height: 1.45;
		color: var(--muted);
	}

	.account :global(.action),
	.account :global(.secondary) {
		padding: 8px 14px;
		border-radius: 10px;
		font-weight: 700;
		font-size: var(--text-sm);
		white-space: nowrap;
	}

	/* The one main action of a form. */
	.account :global(.action) {
		background: var(--accent);
		color: var(--on-accent);
	}

	.account :global(.secondary) {
		background: var(--panel);
		color: var(--text);
		border: 1px solid var(--line);
	}

	.account :global(.action:disabled),
	.account :global(.secondary:disabled) {
		opacity: 0.5;
	}

	.account :global(form) {
		display: grid;
		gap: 6px;
		width: 100%;
	}

	.account :global(label) {
		font-size: var(--text-xs);
		font-weight: 700;
		color: var(--muted);
	}

	.account :global(.row) {
		display: flex;
		gap: 8px;
	}

	.account :global(input) {
		flex: 1;
		min-width: 0;
		padding: 8px 10px;
		border-radius: 10px;
		border: 1px solid var(--line);
		background: var(--panel);
		color: var(--text);
		font: inherit;
		font-size: var(--text-sm);
	}

	/* Codes are typed in capitals; addresses aren't. */
	.account :global(.code-input) {
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}

	.code {
		display: flex;
		align-items: center;
		gap: 10px;
	}

	code {
		font-size: var(--text-lg);
		font-weight: 700;
		letter-spacing: 0.06em;
		user-select: all;
		overflow-wrap: anywhere;
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
		width: 100%;
	}

	li {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		padding: 6px 0;
	}

	li > span {
		display: flex;
		flex-direction: column;
	}

	.danger {
		color: var(--danger);
		font-size: var(--text-sm);
		font-weight: 700;
		padding: 0;
	}
</style>
