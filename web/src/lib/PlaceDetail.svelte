<script lang="ts">
	import ArrowLeft from '@lucide/svelte/icons/arrow-left';
	import CheckIn from './CheckIn.svelte';
	import { closePlace, country } from './state.svelte';
	import { directions, distance, type Place } from './types';

	let { place }: { place: Place | null } = $props();

	let heading = $state<HTMLElement>();
	// Move focus to the details when they open, so screen readers start there.
	$effect(() => heading?.focus());

	/** OSM cuisine tags read as words: "ethiopian;coffee_shop" → "ethiopian, coffee shop". */
	const cuisine = (raw: string) => raw.replaceAll('_', ' ').split(';').map((c) => c.trim()).join(', ');
	const day = (at: number) =>
		new Date(at * 1000).toLocaleDateString(undefined, { weekday: 'short', day: 'numeric', month: 'long', year: 'numeric' });
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && closePlace()} />

<button class="back" onclick={closePlace}><ArrowLeft size={16} strokeWidth={2.5} /> Back</button>

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
			{#if place.hours.status}
				<p class="status" class:open={place.hours.status.open}>{place.hours.status.text}</p>
			{/if}
			{#if place.hours.week}
				<table>
					<tbody>
						{#each place.hours.week as d (d.name)}
							<tr class:today={d.today}>
								<th scope="row">{d.name}</th>
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

	{#if place.osm_url}
		<footer class="muted">
			<p>
				Something wrong? <a href={place.osm_url} target="_blank" rel="noopener">Edit on OpenStreetMap</a>
			</p>
			<p>
				Data © <a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener">OpenStreetMap contributors</a>
			</p>
		</footer>
	{/if}
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
		margin: 12px 0;
	}

	h2 {
		font-size: 24px;
		overflow-wrap: anywhere;
		outline: none;
	}

	h3 {
		font-size: 15px;
		margin: 0 0 6px;
	}

	p {
		margin: 2px 0 0;
		font-size: 14px;
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
		font-size: 14px;
		font-weight: 700;
	}

	.actions a {
		text-decoration: none;
	}

	section {
		margin-top: 18px;
		font-size: 14px;
	}

	.status {
		color: var(--muted);
		font-weight: 700;
		margin-bottom: 6px;
	}

	.status.open,
	.verified {
		color: var(--completed);
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
		font-weight: 800;
	}

	.raw {
		font-family: ui-monospace, monospace;
		font-size: 13px;
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
		font-size: 12px;
	}

	footer a {
		color: inherit;
	}
</style>
