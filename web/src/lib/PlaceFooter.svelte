<script lang="ts">
	import { app, editingOnOsm, refreshPlace, reportClosed } from './state.svelte';
	import type { Place } from './types';

	let { place, confirming = $bindable() }: { place: Place; confirming: boolean } = $props();
</script>

<footer class="muted">
	<p>
		{#if confirming}
			Hide it for you, and for everyone once someone else reports it too?
			<button disabled={app.busyPlace !== null} onclick={() => place && reportClosed(place)}>Yes, it closed</button>
			· <button onclick={() => (confirming = false)}>Cancel</button>
		{:else}
			<button onclick={() => (confirming = true)}>Closed for good?</button>
		{/if}
	</p>
	{#if place.osm_url}
		<p>
			Something wrong?
			<a href={place.osm_url} target="_blank" rel="noopener" onclick={() => place && editingOnOsm(place.id)}>Edit on OpenStreetMap</a>
			·
			<button disabled={app.refreshingPlace !== null} onclick={() => place && refreshPlace(place.id)}>
				{app.refreshingPlace === place.id ? 'Refreshing…' : 'Refresh from OpenStreetMap'}
			</button>
		</p>
		<p>
			Data © <a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener">OpenStreetMap contributors</a>
		</p>
	{/if}
</footer>

<style>
	p {
		margin: 2px 0 0;
		font-size: var(--text-sm);
	}

	footer {
		margin-top: 28px;
		font-size: var(--text-xs);
	}

	footer a,
	footer button {
		color: inherit;
		text-decoration: underline;
		/* A taller hit area than the small text. */
		display: inline-block;
		padding: 12px 0;
		margin: -12px 0;
	}

	footer p + p {
		margin-top: 12px;
	}

	footer button:disabled {
		opacity: 0.6;
	}
</style>
