<script lang="ts">
	import ArrowLeft from '@lucide/svelte/icons/arrow-left';
	import Check from '@lucide/svelte/icons/check';
	import { app, country, toggleVisit } from './state.svelte';
	import { statusOf, visitedToday, type CountryProgress, type Place } from './types';

	let { progress, onback }: { progress: CountryProgress; onback: () => void } = $props();

	const info = $derived(country(progress.code));
	const status = $derived(statusOf(progress));
	let busy = $state<string | null>(null);

	const distance = (m: number) => (m < 1000 ? `${Math.round(m / 10) * 10} m` : `${(m / 1000).toFixed(1)} km`);
	const directions = (lat: number, lng: number) =>
		`https://www.openstreetmap.org/directions?route=%3B${lat}%2C${lng}`;

	// The API sorts open places first, then closed, then unknown hours.
	const groupName = (open: boolean | null) =>
		open === true ? 'Open now' : open === false ? 'Closed' : 'Hours unknown';
	const openNow = (p: Place) => p.hours?.status?.open ?? null;
	const showGroups = $derived(app.places.some((p) => openNow(p) !== null));

	async function toggle(place: Place) {
		busy = place.id;
		await toggleVisit(place);
		busy = null;
	}
</script>

<button class="back" onclick={onback}><ArrowLeft size={16} strokeWidth={2.5} /> All countries</button>

<header>
	<span class="flag" aria-hidden="true">{info?.flag}</span>
	<div>
		<h2>{info?.name ?? progress.code}</h2>
		<p class="muted">
			{#if status === 'completed'}
				Complete: all {progress.places} visited
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
		{#each app.places as place, i (place.id)}
			{@const visited = place.visits.length > 0}
			{@const today = visitedToday(place)}
			{@const now = place.hours?.status}
			{#if showGroups && (i === 0 || openNow(app.places[i - 1]) !== openNow(place))}
				<li class="group">{groupName(openNow(place))}</li>
			{/if}
			<li>
				<div class="info">
					<strong>{place.name}</strong>
					{#if now}
						<span class="hours" class:open={now.open} title={place.hours?.raw}>{now.text}</span>
					{/if}
					<span class="muted">
						{distance(place.distance_m)}{place.address ? ` · ${place.address}` : ''}
						{#if place.visits[0]?.verified}<span class="verified"> · checked in on the spot</span>{/if}
					</span>
					<span class="links">
						<a href={directions(place.lat, place.lng)} target="_blank" rel="noopener">Directions</a>
						{#if place.url}<a href={place.url} target="_blank" rel="noopener">Website</a>{/if}
					</span>
				</div>
				<button
					class="check"
					class:done={today}
					aria-pressed={today}
					disabled={busy === place.id}
					onclick={() => toggle(place)}
				>
					{#if today}<Check size={16} strokeWidth={3} /> Visited{:else if visited}Check in again{:else}Check in{/if}
				</button>
			</li>
		{/each}
	</ul>
{/if}

<style>
	.back {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		color: var(--muted);
		font-size: 14px;
		font-weight: 700;
		padding: 4px 0;
	}

	.back:hover {
		color: var(--text);
	}

	header {
		display: flex;
		align-items: center;
		gap: 14px;
		margin: 12px 0;
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
		font-size: 14px;
	}

	.bar span.discovered {
		background: var(--discovered);
	}

	.bar span.completed {
		background: var(--completed);
	}

	ul {
		list-style: none;
		margin: 8px 0 0;
		padding: 0;
	}

	li {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 14px 0;
		border-bottom: 1px solid var(--line);
	}

	li.group {
		padding: 18px 0 6px;
		color: var(--muted);
		font-size: 13px;
		font-weight: 800;
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}

	.hours {
		color: var(--muted);
		font-weight: 700;
	}

	.hours.open {
		color: var(--completed);
	}

	.info {
		flex: 1;
		min-width: 0;
		display: grid;
		gap: 2px;
		font-size: 14px;
	}

	.info strong {
		font-size: 16px;
		font-weight: 800;
		overflow-wrap: anywhere;
	}

	.links {
		display: flex;
		flex-wrap: wrap;
		gap: 14px;
		margin-top: 2px;
		font-weight: 700;
	}

	.links a {
		text-decoration: none;
	}

	.verified {
		color: var(--completed);
	}

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
