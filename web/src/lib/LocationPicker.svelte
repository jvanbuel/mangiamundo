<script lang="ts">
	import Navigation from '@lucide/svelte/icons/navigation';
	import * as api from './api';
	import { app, locate, setLocation, setRadius, RADII } from './state.svelte';
	import type { GeoResult } from './types';

	let { onclose }: { onclose?: () => void } = $props();

	let query = $state('');
	let results = $state<GeoResult[]>([]);
	let searching = $state(false);
	let searched = $state(false);
	let error = $state<string | null>(null);

	async function search(event: SubmitEvent) {
		event.preventDefault();
		if (query.trim().length < 2) return;
		searching = true;
		error = null;
		try {
			results = await api.geocode(query);
			searched = true;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Search failed.';
		} finally {
			searching = false;
		}
	}

	async function choose(result: GeoResult) {
		onclose?.();
		await setLocation({ lat: result.lat, lng: result.lng, name: result.name.split(',')[0], source: 'search' });
	}

	async function useGps() {
		await locate();
		if (!app.locateError) onclose?.();
	}
</script>

<div class="picker">
	<button class="gps" onclick={useGps} disabled={app.locating}>
		<Navigation size={16} strokeWidth={2.5} aria-hidden="true" />
		{app.locating ? 'Finding you…' : 'Use my current location'}
	</button>
	{#if app.locateError}<p class="error">{app.locateError}</p>{/if}

	<form onsubmit={search}>
		<input
			type="search"
			placeholder="Search a city or address"
			aria-label="Search a city or address"
			bind:value={query}
		/>
		<button type="submit" disabled={searching}>{searching ? '…' : 'Search'}</button>
	</form>
	{#if error}<p class="error">{error}</p>{/if}

	{#if results.length}
		<ul>
			{#each results as result (result.name + result.lat)}
				<li><button onclick={() => choose(result)}>{result.name}</button></li>
			{/each}
		</ul>
	{:else if searched && !searching}
		<p class="muted">No matches. Try a different spelling.</p>
	{/if}

	<div class="radius">
		<span class="muted">Search radius</span>
		<div role="group" aria-label="Search radius">
			{#each RADII as km (km)}
				<button class:on={app.radiusKm === km} aria-pressed={app.radiusKm === km} onclick={() => setRadius(km)}>
					{km} km
				</button>
			{/each}
		</div>
	</div>
</div>

<style>
	.picker {
		display: grid;
		gap: 10px;
	}

	.gps {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 8px;
		padding: 10px 10px 13px;
		border-radius: 12px;
		background: var(--accent);
		color: var(--on-accent);
		font-weight: 800;
		box-shadow: inset 0 -3px 0 color-mix(in srgb, var(--accent) 70%, #000);
	}

	.gps:disabled {
		opacity: 0.7;
	}

	form {
		display: flex;
		gap: 6px;
	}

	input {
		flex: 1;
		min-width: 0;
		padding: 9px 12px;
		border-radius: 10px;
		border: 1px solid var(--line);
		background: var(--bg);
		color: var(--text);
		font: inherit;
	}

	form button {
		padding: 0 14px;
		border-radius: 10px;
		background: var(--panel-raised);
		border: 1px solid var(--line);
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: 2px;
	}

	li button {
		width: 100%;
		text-align: left;
		padding: 8px 10px;
		border-radius: 8px;
	}

	li button:hover {
		background: var(--panel-raised);
	}

	p {
		margin: 0;
		font-size: 13px;
	}

	.error {
		color: var(--danger);
	}

	.radius {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 10px;
		font-size: 13px;
	}

	.radius div {
		display: flex;
		border: 1px solid var(--line);
		border-radius: 9px;
		overflow: hidden;
	}

	.radius button {
		padding: 5px 10px;
	}

	.radius button.on {
		background: var(--panel-raised);
		color: var(--accent);
		font-weight: 700;
	}
</style>
