<script lang="ts">
	import Download from '@lucide/svelte/icons/download';
	import Share2 from '@lucide/svelte/icons/share-2';
	import * as api from './api';
	import Dialog from './Dialog.svelte';
	import { app } from './state.svelte';

	let { onclose }: { onclose: () => void } = $props();

	let file = $state<File | null>(null);
	let url = $state<string | null>(null);
	let error = $state<string | null>(null);

	const dark = $derived(
		app.theme === 'dark' || (app.theme === 'system' && matchMedia('(prefers-color-scheme: dark)').matches)
	);
	const tasted = $derived(app.discover?.world.discovered ?? 0);
	const city = $derived(app.location?.name ?? 'my city');
	const text = $derived(
		tasted
			? `I've tasted ${tasted} ${tasted === 1 ? 'country' : 'countries'} without leaving ${city}. Eat the world with Mangiamundo!`
			: `I'm eating my way around the world in ${city} with Mangiamundo.`
	);
	const canShare = $derived(file !== null && !!navigator.canShare?.({ files: [file] }));

	$effect(() => {
		const here = app.location;
		if (!here) return;
		let cancelled = false;
		api
			.shareCard(here.lat, here.lng, app.radiusKm, here.name, dark ? 'dark' : 'light')
			.then((blob) => {
				if (cancelled) return;
				file = new File([blob], 'mangiamundo.png', { type: 'image/png' });
				url = URL.createObjectURL(blob);
			})
			.catch((e) => {
				if (!cancelled) error = e instanceof Error ? e.message : 'Could not draw the card.';
			});
		return () => {
			cancelled = true;
			if (url) URL.revokeObjectURL(url);
		};
	});

	async function share() {
		if (!file) return;
		try {
			await navigator.share({ files: [file], text });
		} catch {
			// Cancelled by the user, nothing to do.
		}
	}
</script>

<Dialog title="Share your map" {onclose}>
	<div class="preview">
		{#if url}
			<img src={url} alt="Your Mangiamundo card: {text}" />
		{:else if error}
			<p class="muted">{error}</p>
		{:else}
			<p class="muted">Drawing your card…</p>
		{/if}
	</div>
	<div class="actions">
		{#if canShare}
			<button class="primary" onclick={share}><Share2 size={18} strokeWidth={2.5} /> Share</button>
		{/if}
		<a
			class="button"
			class:primary={!canShare}
			class:disabled={!url}
			href={url ?? undefined}
			download="mangiamundo.png"><Download size={18} strokeWidth={2.5} /> Save image</a
		>
	</div>
</Dialog>

<style>
	.preview {
		display: grid;
		place-items: center;
		aspect-ratio: 4 / 5;
		border-radius: 14px;
		background: var(--panel-raised);
		overflow: hidden;
	}

	img {
		width: 100%;
		height: 100%;
		object-fit: contain;
	}

	.actions {
		display: flex;
		gap: 8px;
		margin-top: 14px;
	}

	.actions > * {
		flex: 1;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: 6px;
		padding: 11px 14px 13px;
		border-radius: 12px;
		border: 2px solid var(--line);
		font-weight: 800;
		text-decoration: none;
		color: var(--text);
	}

	.actions .primary {
		--face: var(--accent);
		border: 0;
		background: var(--face);
		color: var(--on-accent);
		box-shadow: inset 0 -3px 0 color-mix(in srgb, var(--face) 70%, #000);
	}

	.disabled {
		pointer-events: none;
		opacity: 0.5;
	}
</style>
