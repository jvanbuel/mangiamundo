<script lang="ts">
	import { app, noCuisine, suggestCuisine, unknownPlaces } from './state.svelte';
	import { distance, type UnknownPlace } from './types';

	let { areaId }: { areaId: string } = $props();

	let open = $state(false);
	let shown = $state(5);
	let places = $state<UnknownPlace[]>([]);
	let busy = $state<string | null>(null);

	$effect(() => {
		areaId;
		places = [];
		shown = 5;
		unknownPlaces().then((found) => (places = found));
	});

	// Only countries with a cuisine can be the food a place serves.
	const countries = $derived(
		(app.catalog?.countries ?? []).filter((c) => c.cuisine).sort((a, b) => a.name.localeCompare(b.name))
	);

	async function answer(place: UnknownPlace, country: string | null) {
		busy = place.id;
		const done = country ? await suggestCuisine(place.id, country) : await noCuisine(place.id);
		busy = null;
		if (done) places = places.filter((p) => p.id !== place.id);
	}
</script>

{#if places.length}
	<section>
		<p class="muted intro">
			{places.length === 1
				? "One restaurant nearby doesn't say what food it serves."
				: `${places.length} restaurants nearby don't say what food they serve.`} <button class="link" aria-expanded={open} onclick={() => (open = !open)}>{open ? 'Hide' : 'Do you know?'}</button>
		</p>
		{#if open}
			<ul>
				{#each places.slice(0, shown) as place (place.id)}
					<li>
						<span class="name">{place.name}</span>
						<span class="muted where">{distance(place.distance_m)}{place.address ? ` · ${place.address}` : ''}</span>
						<div class="answer">
							<select
								aria-label={`Food ${place.name} serves`}
								disabled={busy !== null}
								onchange={(e) => answer(place, e.currentTarget.value)}
							>
								<option value="" selected disabled>Food from…</option>
								{#each countries as c (c.code)}<option value={c.code}>{c.flag} {c.name}</option>{/each}
							</select>
							<button disabled={busy !== null} onclick={() => answer(place, null)}>Nothing specific</button>
						</div>
					</li>
				{/each}
			</ul>
			{#if places.length > shown}
				<p class="more"><button class="link" onclick={() => (shown += 10)}>Show more</button></p>
			{/if}
			<p class="muted small">A country you pick counts for you right away. We ask OpenStreetMap's mappers to add it with a note.</p>
		{/if}
	</section>
{/if}

<style>
	section {
		margin-top: 12px;
	}

	.intro,
	.more {
		text-align: center;
	}

	.link {
		color: inherit;
		text-decoration: underline;
	}

	ul {
		list-style: none;
		margin: 8px 0 0;
		padding: 0;
		display: grid;
		gap: 8px;
	}

	li {
		display: grid;
		gap: 4px;
		padding: 10px 12px;
		border: 1px solid var(--line);
		border-radius: 12px;
		background: var(--panel);
	}

	.name {
		font-weight: 700;
	}

	.where,
	.small {
		font-size: var(--text-sm);
	}

	.small {
		font-size: var(--text-xs);
		text-align: center;
	}

	.answer {
		display: flex;
		gap: 6px;
	}

	select {
		flex: 1;
		min-width: 0;
		padding: 6px 10px;
		border-radius: 10px;
		border: 1px solid var(--line);
		background: var(--panel);
		color: var(--text);
		font: inherit;
	}

	.answer button {
		padding: 6px 10px;
		border-radius: 10px;
		border: 1px solid var(--line);
		font-weight: 700;
		white-space: nowrap;
	}

	button:disabled,
	select:disabled {
		opacity: 0.6;
	}
</style>
