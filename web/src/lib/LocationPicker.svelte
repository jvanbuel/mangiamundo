<script lang="ts">
	import Navigation from '@lucide/svelte/icons/navigation';
	import X from '@lucide/svelte/icons/x';
	import * as api from './api';
	import { app, chooseArea, forgetRecent, locate, switchArea } from './state.svelte';
	import type { Area } from './types';

	let { onclose }: { onclose?: () => void } = $props();

	let query = $state('');
	let results = $state<Area[]>([]);
	let searching = $state(false);
	let searched = $state(false);
	let error = $state<string | null>(null);

	const kindLabel = (area: Area) => (area.kind === 'city' ? 'City' : 'Postal code');
	/** Recent areas other than the current one. */
	const recents = $derived(app.recents.filter((a) => a.id !== app.location?.area.id).slice(0, 5));

	// Suggestions while typing. Failures are ignored: Search still works.
	$effect(() => {
		const text = query.trim();
		searched = false;
		if (text.length < 2) {
			results = [];
			return;
		}
		const near = app.location ? { lat: app.location.lat, lng: app.location.lng } : null;
		let stale = false;
		const timer = setTimeout(async () => {
			const found = await api.suggestAreas(text, near).catch(() => []);
			// An empty list while typing would show "not found" too early.
			if (!stale && !searching && found.length) results = found;
		}, 250);
		return () => {
			stale = true;
			clearTimeout(timer);
		};
	});

	async function search(event: SubmitEvent) {
		event.preventDefault();
		if (query.trim().length < 2) return;
		searching = true;
		error = null;
		try {
			results = await api.searchAreas(query);
			searched = true;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Search failed.';
		} finally {
			searching = false;
		}
	}

	async function choose(area: Area) {
		onclose?.();
		await chooseArea(area);
	}

	async function useGps() {
		await locate();
		if (!app.locateError) onclose?.();
	}

	async function pick(area: Area) {
		onclose?.();
		await switchArea(area);
	}
</script>

<div class="picker">
	{#if app.location && app.location.options.length > 1}
		<div class="options" role="group" aria-label="Area around you">
			{#each app.location.options as option (option.id)}
				<button
					class:on={app.location.area.id === option.id}
					aria-pressed={app.location.area.id === option.id}
					onclick={() => pick(option)}
				>
					<span>{option.kind === 'city' ? `All of ${option.name}` : `Only ${option.name}`}</span>
				</button>
			{/each}
		</div>
	{/if}

	<button class="gps" onclick={useGps} disabled={app.locating}>
		<Navigation size={16} strokeWidth={2.5} aria-hidden="true" />
		{app.locating ? 'Finding you…' : 'Use my current location'}
	</button>
	{#if app.locateError}<p class="error">{app.locateError}</p>{/if}

	<form onsubmit={search}>
		<input
			type="search"
			placeholder="City or postal code"
			aria-label="Search a city or postal code"
			bind:value={query}
		/>
		<button type="submit" disabled={searching}>{searching ? '…' : 'Search'}</button>
	</form>
	{#if error}<p class="error">{error}</p>{/if}

	{#if results.length}
		<ul>
			{#each results as result (result.id)}
				<li>
					<button onclick={() => choose(result)}>
						<span class="name">{result.name}</span>
						<span class="muted">{kindLabel(result)}{result.detail ? ` · ${result.detail}` : ''}</span>
					</button>
				</li>
			{/each}
		</ul>
	{:else if searched && !searching}
		<p class="muted">No city or postal code found. Try a different spelling.</p>
	{:else if !query.trim() && recents.length}
		<section aria-labelledby="recent-heading">
			<h3 id="recent-heading">Recent</h3>
			<ul>
				{#each recents as area (area.id)}
					<li class="recent">
						<button onclick={() => choose(area)}>
							<span class="name">{area.name}</span>
							<span class="muted">{kindLabel(area)}{area.detail ? ` · ${area.detail}` : ''}</span>
						</button>
						<button class="forget" aria-label="Remove {area.name} from recent" onclick={() => forgetRecent(area.id)}>
							<X size={16} strokeWidth={2.5} aria-hidden="true" />
						</button>
					</li>
				{/each}
			</ul>
		</section>
	{/if}
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
		font-weight: 700;
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
		background: var(--panel);
		color: var(--text);
		font: inherit;
	}

	form button {
		padding: 0 14px;
		border-radius: 10px;
		background: var(--panel);
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
		background: var(--panel);
	}

	p {
		margin: 0;
		font-size: var(--text-sm);
	}

	.error {
		color: var(--danger);
	}

	li button {
		display: grid;
		gap: 1px;
	}

	li .muted {
		font-size: var(--text-xs);
	}

	h3 {
		margin: 4px 0 4px 10px;
		font-size: var(--text-xs);
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		color: var(--muted);
	}

	li.recent {
		display: flex;
		align-items: center;
	}

	li.recent button:first-child {
		flex: 1;
		min-width: 0;
	}

	li button.forget {
		width: auto;
		display: grid;
		place-items: center;
		padding: 14px;
		color: var(--muted);
	}

	li button.forget:hover {
		color: var(--text);
	}

	.options {
		display: flex;
		border: 1px solid var(--line);
		border-radius: 10px;
		overflow: hidden;
	}

	.options button {
		flex: 1;
		padding: 8px 10px;
		font-size: var(--text-sm);
	}

	.options button + button {
		border-left: 1px solid var(--line);
	}

	.options button.on {
		background: var(--panel);
		color: var(--accent);
		font-weight: 700;
	}
</style>
