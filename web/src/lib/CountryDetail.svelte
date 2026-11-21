<script lang="ts">
	import ArrowLeft from '@lucide/svelte/icons/arrow-left';
	import CheckIn from './CheckIn.svelte';
	import { app, country, openPlace } from './state.svelte';
	import { directions, distance, statusOf, type CountryProgress, type Place } from './types';

	let { progress, onback }: { progress: CountryProgress; onback: () => void } = $props();

	const info = $derived(country(progress.code));
	const status = $derived(statusOf(progress));

	// The API sorts open places first, then closed, then unknown hours.
	const groupName = (open: boolean | null) =>
		open === true ? 'Open now' : open === false ? 'Closed' : 'Hours unknown';
	const openNow = (p: Place) => p.hours?.status?.open ?? null;
	const showGroups = $derived(app.places.some((p) => openNow(p) !== null));

	/** Gives focus back to the link of the place whose details just closed. */
	const refocus = (id: string) => (link: HTMLElement) => {
		if (app.closedPlace !== id) return;
		app.closedPlace = null;
		link.focus();
	};

	function open(event: MouseEvent, place: Place) {
		// Let modified clicks open the link in a new tab.
		if (event.metaKey || event.ctrlKey || event.shiftKey || event.button !== 0) return;
		event.preventDefault();
		void openPlace(place.id);
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
			{@const now = place.hours?.status}
			{#if showGroups && (i === 0 || openNow(app.places[i - 1]) !== openNow(place))}
				<li class="group">{groupName(openNow(place))}</li>
			{/if}
			<li>
				<div class="info">
					<a class="name" href="?place={encodeURIComponent(place.id)}" onclick={(e) => open(e, place)} {@attach refocus(place.id)}>{place.name}</a>
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
				<CheckIn {place} />
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
		font-size: var(--text-sm);
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
		font-size: var(--text-2xl);
	}

	p {
		margin: 2px 0 0;
		font-size: var(--text-sm);
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
		position: relative;
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 14px 0;
		border-bottom: 1px solid var(--line);
	}

	li.group {
		padding: 18px 0 6px;
		color: var(--muted);
		font-size: var(--text-sm);
		font-weight: 700;
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
		font-size: var(--text-sm);
	}

	.name {
		color: inherit;
		text-decoration: none;
		font-size: var(--text-base);
		font-weight: 700;
		overflow-wrap: anywhere;
	}

	.name:hover {
		text-decoration: underline;
	}

	/* The whole card opens the details; the links and the button on it stay on top. */
	.name::after {
		content: '';
		position: absolute;
		inset: 0;
	}

	.links a,
	li :global(.check) {
		position: relative;
		z-index: 1;
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
</style>
