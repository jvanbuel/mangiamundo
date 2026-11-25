<script lang="ts">
	import ArrowLeft from '@lucide/svelte/icons/arrow-left';
	import Dices from '@lucide/svelte/icons/dices';
	import CheckIn from './CheckIn.svelte';
	import OpeningHours from './OpeningHours.svelte';
	import PlaceFooter from './PlaceFooter.svelte';
	import ShareButton from './ShareButton.svelte';
	import { shareVisit } from './api';
	import { app, closePlace, country, notTodaySpin, spin } from './state.svelte';
	import { directions, distance, type Place } from './types';

	let { place }: { place: Place | null } = $props();

	let heading = $state<HTMLElement>();
	// Reports cannot be taken back, so the first press only asks.
	let confirming = $state(false);
	// Move focus to the details when they open, so screen readers start there.
	$effect(() => heading?.focus());

	/** OSM cuisine tags read as words: "ethiopian;coffee_shop" → "Ethiopian, coffee shop". */
	const cuisine = (raw: string) => {
		const words = raw.replaceAll('_', ' ').split(';').map((c) => c.trim()).join(', ');
		return words.charAt(0).toUpperCase() + words.slice(1);
	};
	/** Where Back goes: the country the place was opened from, if any. */
	const from = $derived(app.selected ? country(app.selected)?.name : null);
	const day = (at: number) =>
		new Date(at * 1000).toLocaleDateString(undefined, { weekday: 'short', day: 'numeric', month: 'long', year: 'numeric' });
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && closePlace()} />

<button class="back" onclick={closePlace}><ArrowLeft size={16} strokeWidth={2.5} /> {from ?? 'Back'}</button>

{#if !place}
	<p class="muted">Loading place…</p>
{:else}
	<header>
		<h2 tabindex="-1" bind:this={heading}>{place.name}</h2>
		{#if place.cuisine}<p class="muted">{cuisine(place.cuisine)}</p>{/if}
		<p class="flags">
			{#each place.countries as code (code)}
				{@const info = country(code)}
				<span>{info?.flag} {info?.name ?? code}</span>
			{/each}
		</p>
	</header>

	<div class="actions">
		<CheckIn {place} />
		<a href={directions(place.lat, place.lng)} target="_blank" rel="noopener">Directions</a>
		{#if place.url}<a href={place.url} target="_blank" rel="noopener">Website</a>{/if}
	</div>

	{#if app.spun === place.id}
		<div class="spun">
			<button disabled={app.spinning} onclick={spin}><Dices size={16} strokeWidth={2.5} aria-hidden="true" /> Spin again</button>
			<button disabled={app.spinning} onclick={() => place && notTodaySpin(place.id)}>Not today</button>
		</div>
	{/if}

	{@const where = [place.address, place.distance_m > 0 ? `${distance(place.distance_m)} away` : null]}
	{#if where.some(Boolean)}<p>{where.filter(Boolean).join(' · ')}</p>{/if}

	{#if place.hours}<OpeningHours hours={place.hours} />{/if}

	<section>
		<h3>Your visits</h3>
		{#if place.visits.length}
			<ul>
				{#each place.visits as visit (visit.id)}
					<li class="visit">
						<span>
							{day(visit.at)}
							{#if visit.verified}<span class="verified"> · checked in on the spot</span>{/if}
						</span>
						<ShareButton label="Share" link={() => shareVisit(visit.id)} />
					</li>
				{/each}
			</ul>
		{:else}
			<p class="muted">Not visited yet.</p>
		{/if}
	</section>

	<PlaceFooter {place} bind:confirming />
{/if}

<style>
	.visit {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 10px;
		padding: 3px 0;
	}

	.back {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		color: var(--muted);
		font-size: var(--text-sm);
		font-weight: 700;
		padding: 12px 8px 12px 0;
		margin: -8px 0;
	}

	.back:hover {
		color: var(--text);
	}

	header {
		margin: 12px 0;
	}

	h2 {
		font-size: var(--text-2xl);
		overflow-wrap: anywhere;
		outline: none;
	}

	h3 {
		font-size: var(--text-base);
		margin: 0 0 6px;
	}

	p {
		margin: 2px 0 0;
		font-size: var(--text-sm);
	}

	.flags {
		display: flex;
		flex-wrap: wrap;
		gap: 4px 12px;
		font-weight: 700;
	}

	.actions {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 16px;
		margin-bottom: 10px;
		font-size: var(--text-sm);
		font-weight: 700;
	}

	.actions a {
		text-decoration: none;
		padding: 12px 0;
	}

	/* Small chunky buttons, like the check-in one but on the panel colour. */
	.spun {
		display: flex;
		gap: 8px;
		margin-bottom: 10px;
	}

	.spun button {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		padding: 6px 12px 8px;
		border-radius: 10px;
		background: var(--glass);
		border: 1px solid var(--line);
		font-size: var(--text-sm);
		font-weight: 700;
		box-shadow: inset 0 -3px 0 var(--line);
	}

	.spun button:active {
		padding: 7px 12px;
		box-shadow: none;
	}

	.spun button:disabled {
		opacity: 0.6;
	}

	section {
		margin-top: 18px;
		font-size: var(--text-sm);
	}

	.verified {
		color: var(--discovered-text);
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	li {
		padding: 6px 0;
		border-bottom: 1px solid var(--line);
	}
</style>
