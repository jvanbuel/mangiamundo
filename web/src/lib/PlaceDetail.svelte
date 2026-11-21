<script lang="ts">
	import ArrowLeft from '@lucide/svelte/icons/arrow-left';
	import CheckIn from './CheckIn.svelte';
	import { app, closePlace, country, editingOnOsm, refreshPlace, reportClosed } from './state.svelte';
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
	/** Open around the clock every day: one line says it better than seven. */
	const allDay = $derived(place?.hours?.week?.every((d) => d.open.length === 1 && d.open[0] === '24 hours') ?? false);
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

	{@const where = [place.address, place.distance_m > 0 ? `${distance(place.distance_m)} away` : null]}
	{#if where.some(Boolean)}<p>{where.filter(Boolean).join(' · ')}</p>{/if}

	{#if place.hours}
		<section>
			<h3>Opening hours</h3>
			{#if allDay}
				<p class="status open">Open 24 hours, every day</p>
			{:else if place.hours.status}
				<p class="status" class:open={place.hours.status.open}>{place.hours.status.text}</p>
			{/if}
			{#if allDay}
				<!-- The line above says it all. -->
			{:else if place.hours.week}
				<table>
					<tbody>
						{#each place.hours.week as d (d.name)}
							<tr class:today={d.today}>
								<th scope="row">{d.name}{#if d.today}<span class="sr-only"> (today)</span>{/if}</th>
								<td>{d.open.length ? d.open.join(', ') : 'Closed'}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			{:else}
				<p class="muted raw">{place.hours.raw}</p>
			{/if}
		</section>
	{/if}

	<section>
		<h3>Your visits</h3>
		{#if place.visits.length}
			<ul>
				{#each place.visits as visit (visit.id)}
					<li>
						{day(visit.at)}
						{#if visit.verified}<span class="verified"> · checked in on the spot</span>{/if}
					</li>
				{/each}
			</ul>
		{:else}
			<p class="muted">Not visited yet.</p>
		{/if}
	</section>

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
{/if}

<style>
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

	section {
		margin-top: 18px;
		font-size: var(--text-sm);
	}

	.status {
		color: var(--muted);
		font-weight: 700;
		margin-bottom: 6px;
	}

	.status.open {
		color: var(--text);
	}

	.verified {
		color: var(--accent-text);
	}

	table {
		border-collapse: collapse;
	}

	th,
	td {
		padding: 3px 16px 3px 0;
		text-align: left;
		vertical-align: top;
		font-weight: 500;
	}

	tr.today th,
	tr.today td {
		font-weight: 700;
	}

	.sr-only {
		position: absolute;
		width: 1px;
		height: 1px;
		overflow: hidden;
		clip-path: inset(50%);
		white-space: nowrap;
	}

	.raw {
		font-family: ui-monospace, monospace;
		font-size: var(--text-sm);
		overflow-wrap: anywhere;
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
