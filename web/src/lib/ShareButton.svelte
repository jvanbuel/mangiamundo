<script lang="ts">
	import Share from '@lucide/svelte/icons/share-2';
	import { prepare, share, type Prepared } from './share';
	import { app, toast } from './state.svelte';
	import type { ShareLink } from './types';

	let { label, link }: { label: string; link: () => Promise<ShareLink> } = $props();

	let busy = $state(false);
	/** Kept when the browser wanted a fresh tap; the next tap shares it at once. */
	let ready = $state<Prepared | null>(null);
	/** The public link as it was when `ready` was made. */
	let readyAt = 0;

	async function go() {
		// A link turned off (or made again) since then may be dead: make a fresh one.
		if (ready && readyAt !== app.publicLink) ready = null;
		if (ready) {
			const prepared = ready;
			ready = null;
			await share(prepared);
			return;
		}
		busy = true;
		try {
			const made = await link();
			// After making it (which may itself change the public link), before the waits in
			// which it could be turned off.
			const at = app.publicLink;
			const prepared = await prepare(made);
			if (!(await share(prepared))) {
				ready = prepared;
				readyAt = at;
			}
		} catch (e) {
			toast(null, 'Could not share', (e as Error).message);
		} finally {
			busy = false;
		}
	}
</script>

<button class="share" disabled={busy} onclick={go}>
	<Share size={14} strokeWidth={2.5} />
	{busy ? 'Preparing…' : ready ? 'Share now' : label}
</button>

<style>
	/* A small chunky button, like the check-in one. */
	.share {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 5px 11px 7px;
		border-radius: 9px;
		background: var(--accent);
		color: var(--on-accent);
		font-weight: 700;
		font-size: var(--text-sm);
		box-shadow: inset 0 -2px 0 color-mix(in srgb, var(--accent) 70%, #000);
	}

	.share:active {
		padding: 6px 11px;
		box-shadow: none;
	}

	.share:disabled {
		opacity: 0.6;
	}
</style>
