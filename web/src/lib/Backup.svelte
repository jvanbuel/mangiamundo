<script lang="ts">
	import Copy from '@lucide/svelte/icons/copy';
	import { renderSVG } from 'uqr';
	import * as api from './api';
	import Dialog from './Dialog.svelte';
	import { app, restorePassport } from './state.svelte';

	let { onclose }: { onclose: () => void } = $props();

	let code = $state<string | null>(null);
	let error = $state<string | null>(null);
	let copied = $state(false);
	let typed = $state(app.pendingRestore ?? '');
	let busy = $state(false);

	/** Scanning the QR code with another phone's camera opens the app there and offers to restore. */
	const link = $derived(code ? `${location.origin}/?restore=${code}` : null);
	const qr = $derived(link ? renderSVG(link, { border: 2, whiteColor: '#ffffff', blackColor: '#14263a' }) : null);
	const visits = $derived(app.discover?.profile.visits ?? 0);

	$effect(() => {
		// Arriving from a QR code: show only the restore step, not this device's own code.
		if (app.pendingRestore) return;
		api
			.recoveryCode()
			.then((r) => (code = r.code))
			.catch((e) => (error = e instanceof Error ? e.message : 'Could not load your code.'));
	});

	async function copy() {
		if (!code) return;
		await navigator.clipboard.writeText(code);
		copied = true;
		setTimeout(() => (copied = false), 2000);
	}

	async function rotate() {
		if (!confirm('Make a new code? The old code and QR code stop working.')) return;
		code = (await api.rotateRecoveryCode()).code;
	}

	async function restore(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		if (await restorePassport(typed)) {
			app.pendingRestore = null;
			onclose();
		}
		busy = false;
	}

	function close() {
		app.pendingRestore = null;
		onclose();
	}
</script>

<Dialog title={app.pendingRestore ? 'Open this passport here?' : 'Back up your passport'} onclose={close}>
	{#if !app.pendingRestore}
		<p class="muted">
			Your progress is tied to this browser. Keep this code somewhere safe to get it back on a new phone or
			computer. Anyone with the code can open your passport.
		</p>
		{#if error}
			<p class="error">{error}</p>
		{:else if code}
			<div class="code">
				<div class="qr" aria-hidden="true">{@html qr}</div>
				<div>
					<p class="label">Recovery code</p>
					<p class="digits">{code}</p>
					<div class="row">
						<button class="link" onclick={copy}><Copy size={14} strokeWidth={2.5} /> {copied ? 'Copied' : 'Copy'}</button>
						<button class="link" onclick={rotate}>New code</button>
					</div>
				</div>
			</div>
			<p class="muted small">On another device, scan the QR code with the camera, or enter the code below there.</p>
		{:else}
			<p class="muted">Loading your code…</p>
		{/if}
		<hr />
		<h3>Restore from another device</h3>
	{:else}
		<p class="muted">
			This switches this browser to the passport behind the code.
			{#if visits}The {visits} check-ins made here are added to it.{/if}
		</p>
	{/if}
	<form onsubmit={restore}>
		<input
			bind:value={typed}
			placeholder="XXXX-XXXX-XXXX-XXXX"
			autocomplete="off"
			autocapitalize="characters"
			spellcheck="false"
			aria-label="Recovery code"
		/>
		<button class="primary" disabled={busy || typed.replace(/[^0-9a-z]/gi, '').length !== 16}>
			{busy ? 'Restoring…' : 'Restore'}
		</button>
	</form>
</Dialog>

<style>
	p {
		margin: 0 0 12px;
	}

	.small {
		font-size: 13px;
	}

	.error {
		color: var(--danger);
	}

	.code {
		display: flex;
		align-items: center;
		gap: 16px;
		margin-bottom: 12px;
	}

	.qr {
		flex: none;
		width: 132px;
		height: 132px;
		border-radius: 12px;
		overflow: hidden;
		border: 2px solid var(--line);
	}

	.qr :global(svg) {
		display: block;
		width: 100%;
		height: 100%;
	}

	.label {
		margin: 0;
		font-size: 13px;
		color: var(--muted);
		font-weight: 700;
	}

	.digits {
		margin: 2px 0 6px;
		font-family: ui-monospace, 'SF Mono', Menlo, monospace;
		font-size: 19px;
		font-weight: 700;
		letter-spacing: 0.04em;
		overflow-wrap: anywhere;
	}

	.row {
		display: flex;
		gap: 16px;
	}

	.link {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		color: var(--accent);
		font-weight: 800;
		font-size: 14px;
	}

	hr {
		border: 0;
		border-top: 1px solid var(--line);
		margin: 16px 0;
	}

	h3 {
		font-size: 15px;
		font-weight: 800;
		margin-bottom: 8px;
	}

	form {
		display: flex;
		gap: 8px;
	}

	input {
		flex: 1;
		min-width: 0;
		padding: 10px 12px;
		border-radius: 10px;
		border: 2px solid var(--line);
		background: var(--bg);
		color: var(--text);
		font: inherit;
		font-family: ui-monospace, 'SF Mono', Menlo, monospace;
		text-transform: uppercase;
	}

	.primary {
		--face: var(--accent);
		padding: 10px 16px 12px;
		border-radius: 12px;
		background: var(--face);
		color: var(--on-accent);
		font-weight: 800;
		box-shadow: inset 0 -3px 0 color-mix(in srgb, var(--face) 70%, #000);
	}

	.primary:disabled {
		opacity: 0.5;
		cursor: default;
	}
</style>
