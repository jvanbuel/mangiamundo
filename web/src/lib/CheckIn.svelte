<script lang="ts">
	import Check from '@lucide/svelte/icons/check';
	import { toggleVisit } from './state.svelte';
	import { visitedToday, type Place } from './types';

	let { place }: { place: Place } = $props();

	const today = $derived(visitedToday(place));
	let busy = $state(false);

	async function toggle() {
		busy = true;
		await toggleVisit(place);
		busy = false;
	}
</script>

<button class="check" class:done={today} aria-pressed={today} disabled={busy} onclick={toggle}>
	{#if today}<Check size={16} strokeWidth={3} /> Visited{:else if place.visits.length}Check in again{:else}Check in{/if}
</button>

<style>
	/* Chunky, pressable buttons: a solid face on a darker lower edge. */
	.check {
		--face: var(--accent);
		flex: none;
		display: inline-flex;
		align-items: center;
		gap: 4px;
		padding: 9px 14px 11px;
		border-radius: 12px;
		background: var(--face);
		color: var(--on-accent);
		font-size: 14px;
		font-weight: 800;
		box-shadow: inset 0 -3px 0 color-mix(in srgb, var(--face) 70%, #000);
	}

	.check:active {
		padding: 10px 14px;
		box-shadow: none;
	}

	.check.done {
		--face: var(--completed);
		color: var(--on-completed);
	}

	.check:disabled {
		opacity: 0.6;
	}
</style>
