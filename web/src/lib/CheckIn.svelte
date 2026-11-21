<script lang="ts">
	import Check from '@lucide/svelte/icons/check';
	import { app, toggleVisit } from './state.svelte';
	import { visitedToday, type Place } from './types';

	let { place }: { place: Place } = $props();

	// Ticks so "Visited" turns back into "Check in again" after midnight.
	let now = $state(new Date());
	$effect(() => {
		const timer = setInterval(() => (now = new Date()), 60_000);
		return () => clearInterval(timer);
	});
	const today = $derived(visitedToday(place, now));

	function toggle() {
		now = new Date();
		// Past midnight the label just changed: let the user see it before acting.
		if (visitedToday(place, now) !== today) return;
		void toggleVisit(place);
	}
</script>

<button class="check" class:done={today} aria-pressed={today} disabled={app.busyPlace !== null} onclick={toggle}>
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
		font-size: var(--text-sm);
		font-weight: 700;
		box-shadow: inset 0 -3px 0 color-mix(in srgb, var(--face) 70%, #000);
	}

	.check:active {
		padding: 10px 14px;
		box-shadow: none;
	}

	.check.done {
		background: color-mix(in srgb, var(--discovered) 20%, var(--panel));
		color: var(--discovered-text);
		box-shadow: none;
		padding: 10px 14px;
	}

	.check:disabled {
		opacity: 0.6;
	}
</style>
