<script lang="ts">
	import { app, country, toggleVisit } from './state.svelte';
	import { statusOf, type CountryProgress } from './types';

	let { progress, onback }: { progress: CountryProgress; onback: () => void } = $props();

	const info = $derived(country(progress.code));
	const status = $derived(statusOf(progress));
	let busy = $state<string | null>(null);

	const distance = (m: number) => (m < 1000 ? `${Math.round(m / 10) * 10} m` : `${(m / 1000).toFixed(1)} km`);
	const directions = (lat: number, lng: number) =>
		`https://www.google.com/maps/dir/?api=1&destination=${lat},${lng}`;

	async function toggle(place: (typeof app.places)[number]) {
		busy = place.id;
		await toggleVisit(place);
		busy = null;
	}
</script>

<button class="back" onclick={onback}>← All countries</button>

<header>
	<span class="flag" aria-hidden="true">{info?.flag}</span>
	<div>
		<h2>{info?.name ?? progress.code}</h2>
		<p class="muted">
			{#if status === 'completed'}
				Completed · all {progress.places} visited
			{:else}
				{progress.visited} of {progress.places} {progress.places === 1 ? 'place' : 'places'} visited
			{/if}
		</p>
	</div>
</header>
<div class="bar"><span class={status} style:width="{(progress.visited / progress.places) * 100}%"></span></div>

{#if app.placesLoading}
	<p class="muted">Loading places…</p>
{:else}
	<ul>
		{#each app.places as place (place.id)}
			<li class:visited={place.visited_at !== null}>
				<div class="info">
					<strong>{place.name}</strong>
					<span class="muted">
						{distance(place.distance_m)}{place.address ? ` · ${place.address}` : ''}
					</span>
					<span class="links">
						<a href={directions(place.lat, place.lng)} target="_blank" rel="noopener">Directions</a>
						{#if place.url}<a href={place.url} target="_blank" rel="noopener">Website</a>{/if}
						{#if place.verified}<span class="verified" title="Checked in on the spot">✓ verified</span>{/if}
					</span>
				</div>
				<button
					class="check"
					aria-pressed={place.visited_at !== null}
					disabled={busy === place.id}
					onclick={() => toggle(place)}
				>
					{place.visited_at !== null ? '✓ Visited' : 'Check in'}
				</button>
			</li>
		{/each}
	</ul>
{/if}

<style>
	.back {
		color: var(--muted);
		font-size: 13px;
		padding: 4px 0;
	}

	.back:hover {
		color: var(--text);
	}

	header {
		display: flex;
		align-items: center;
		gap: 14px;
		margin: 10px 0 12px;
	}

	.flag {
		font-size: 44px;
		line-height: 1;
	}

	h2 {
		font-size: 24px;
	}

	p {
		margin: 2px 0 0;
		font-size: 13px;
	}

	.bar span.discovered {
		background: var(--discovered);
	}

	.bar span.completed {
		background: var(--completed);
	}

	ul {
		list-style: none;
		margin: 16px 0 0;
		padding: 0;
		display: grid;
		gap: 8px;
	}

	li {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 12px;
		border-radius: 12px;
		background: var(--panel-raised);
		border: 1px solid transparent;
	}

	li.visited {
		border-color: color-mix(in srgb, var(--completed) 45%, transparent);
	}

	.info {
		flex: 1;
		min-width: 0;
		display: grid;
		gap: 2px;
		font-size: 13px;
	}

	.info strong {
		font-size: 15px;
		overflow-wrap: anywhere;
	}

	.links {
		display: flex;
		flex-wrap: wrap;
		gap: 12px;
		margin-top: 2px;
	}

	.verified {
		color: var(--completed);
	}

	.check {
		flex: none;
		padding: 8px 12px;
		border-radius: 999px;
		border: 1px solid var(--accent);
		color: var(--accent);
		font-size: 13px;
		font-weight: 600;
	}

	.check[aria-pressed='true'] {
		background: var(--completed);
		border-color: var(--completed);
		color: var(--on-completed);
	}

	.check:disabled {
		opacity: 0.6;
	}
</style>
