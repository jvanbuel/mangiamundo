<script lang="ts">
	import { onMount } from 'svelte';
	import * as api from './api';
	import { accountChanged, toast } from './state.svelte.ts';
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

	async function load() {
		try {
			account = await api.account();
			error = null;
		} catch (e) {
			error = message(e);
		}
	}

	onMount(() => {
		void load();
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

	const ago = (at: number) => {
		const days = Math.floor((now - at) / 86400);
		if (days < 1) return 'today';
		if (days === 1) return 'yesterday';
		return days < 60 ? `${days} days ago` : new Date(at * 1000).toLocaleDateString();
	};
</script>

<section class="account">
	<h3>Your progress</h3>
	<p class="muted">It lives on this device. Keep a recovery code to get it back after clearing your browser or on a new phone.</p>

	{#if recoveryCode}
		<div class="code" aria-live="polite">
			<code>{recoveryCode}</code>
			<button onclick={() => copy(recoveryCode!)}>Copy</button>
		</div>
		<p class="muted small">Shown only now. Anyone with this code can use your account.</p>
	{:else}
		<button class="action" disabled={busy || !account} onclick={makeRecoveryCode}>
			{account?.recovery_code_created_at ? 'Make a new recovery code' : 'Make a recovery code'}
		</button>
		{#if account?.recovery_code_created_at}
			<p class="muted small">You made one {ago(account.recovery_code_created_at)}.</p>
		{/if}
	{/if}

	<h3>Use it on another device</h3>
	{#if link}
		<div class="code" aria-live="polite">
			<code>{link.code}</code>
			<button onclick={() => copy(link!.code)}>Copy</button>
		</div>
		<p class="muted small">Enter it there under Settings within {linkLeft} {linkLeft === 1 ? 'minute' : 'minutes'}. It works once.</p>
	{:else}
		<button class="action" disabled={busy} onclick={makeLinkCode}>Show a link code</button>
	{/if}

	<form
		onsubmit={(e) => {
			e.preventDefault();
			if (entered.trim()) void restore();
		}}
	>
		<label for="restore-code">Have a recovery or link code?</label>
		<div class="row">
			<input id="restore-code" bind:value={entered} placeholder="XXXX-XXXX" autocomplete="off" autocapitalize="characters" spellcheck="false" />
			<button class="action" disabled={busy || !entered.trim()}>Use</button>
		</div>
		<p class="muted small">What you did on this device so far is added to that account.</p>
	</form>

	{#if error}
		<p class="muted small">{error}</p>
	{:else if account && account.devices.length > 1}
		<h3>Signed-in devices</h3>
		<ul>
			{#each account.devices as device (device.id)}
				<li>
					<span>
						<strong>{device.name ?? 'Unnamed device'}</strong>
						<span class="muted small">{device.current ? 'This device' : `Used ${ago(device.last_used_at)}`}</span>
					</span>
					{#if !device.current}
						<button disabled={busy} onclick={() => signOut(device.id)}>Sign out</button>
					{/if}
				</li>
			{/each}
		</ul>
	{/if}

	<button class="danger" disabled={busy} onclick={deleteEverything}>Delete my account</button>
</section>

<style>
	.account {
		margin: 4px 0 14px;
		padding-top: 12px;
		border-top: 1px solid var(--line);
	}

	h3 {
		font-size: var(--text-sm);
		margin: 14px 0 6px;
	}

	h3:first-child {
		margin-top: 0;
	}

	.account p {
		margin: 0 0 8px;
	}

	.small {
		font-size: var(--text-xs);
	}

	.action {
		padding: 8px 14px;
		border-radius: 10px;
		background: var(--accent);
		color: var(--on-accent);
		font-weight: 700;
		font-size: var(--text-sm);
	}

	.action:disabled {
		opacity: 0.6;
	}

	.code {
		display: flex;
		align-items: center;
		gap: 10px;
		margin-bottom: 6px;
	}

	code {
		font-size: var(--text-lg);
		font-weight: 700;
		letter-spacing: 0.06em;
		user-select: all;
		overflow-wrap: anywhere;
	}

	.code button,
	li button {
		color: var(--accent);
		font-weight: 700;
		font-size: var(--text-sm);
		padding: 6px;
	}

	form {
		margin-top: 14px;
	}

	label {
		display: block;
		font-weight: 700;
		font-size: var(--text-sm);
		margin-bottom: 6px;
	}

	.row {
		display: flex;
		gap: 8px;
		margin-bottom: 6px;
	}

	input {
		flex: 1;
		min-width: 0;
		padding: 8px 10px;
		border-radius: 10px;
		border: 1px solid var(--line);
		background: var(--panel);
		color: var(--text);
		font: inherit;
		text-transform: uppercase;
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
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
		margin-top: 14px;
		color: var(--muted);
		font-size: var(--text-sm);
		text-decoration: underline;
	}
</style>
