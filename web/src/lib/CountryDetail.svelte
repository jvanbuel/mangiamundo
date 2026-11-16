<script lang="ts">
	import ArrowLeft from '@lucide/svelte/icons/arrow-left';
	import Check from '@lucide/svelte/icons/check';
	import ChefHat from '@lucide/svelte/icons/chef-hat';
	import Repeat from '@lucide/svelte/icons/repeat';
	import { app, checkIn, country, dishesOf, logDish, undoCheckIn } from './state.svelte';
	import { statusOf, type CountryProgress, type Dish, type Place } from './types';

	let { progress, onback }: { progress: CountryProgress; onback: () => void } = $props();

	const info = $derived(country(progress.code));
	const status = $derived(statusOf(progress));
	let busy = $state<string | null>(null);
	/** Ticks every minute so cooldowns run out without a reload. */
	let now = $state(Date.now() / 1000);
	$effect(() => {
		const timer = setInterval(() => (now = Date.now() / 1000), 60_000);
		return () => clearInterval(timer);
	});

	const signature = $derived(dishesOf([progress.code]));
	const tried = $derived(new Set(app.discover?.dishes_tried ?? []));

	const distance = (m: number) => (m < 1000 ? `${Math.round(m / 10) * 10} m` : `${(m / 1000).toFixed(1)} km`);
	const directions = (lat: number, lng: number) =>
		`https://www.google.com/maps/dir/?api=1&destination=${lat},${lng}`;

	const day = (unix: number) =>
		new Date(unix * 1000).toLocaleDateString(undefined, { day: 'numeric', month: 'short' });

	/** Checked in within the cooldown: the time to log a dish or take it back. */
	const recent = (place: Place) => place.next_check_in_at !== null && place.next_check_in_at > now;

	async function run(place: Place, action: () => Promise<void>) {
		busy = place.id;
		await action();
		busy = null;
	}

	function pick(place: Place, dish: Dish) {
		void run(place, () => logDish(place, place.last_dish === dish.id ? null : dish));
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

{#if signature.length}
	<section class="passport" aria-label="Signature dishes">
		<h3><ChefHat size={16} strokeWidth={2.5} aria-hidden="true" /> Signature dishes</h3>
		<ul class="chips">
			{#each signature as dish (dish.id)}
				<li class:tried={tried.has(dish.id)}>
					{#if tried.has(dish.id)}<Check size={14} strokeWidth={3} aria-label="Tasted" />{/if}
					{dish.name}
				</li>
			{/each}
		</ul>
		<p class="muted hint">Log one when you check in: +25 XP for each new dish.</p>
	</section>
{/if}

{#if app.placesLoading}
	<p class="muted">Loading places…</p>
{:else}
	<ul class="places">
		{#each app.places as place (place.id)}
			{@const visited = place.visited_at !== null}
			{@const waiting = recent(place)}
			<li>
				<div class="row">
					<div class="info">
						<strong>{place.name}</strong>
						<span class="muted">
							{distance(place.distance_m)}{place.address ? ` · ${place.address}` : ''}
						</span>
						{#if visited}
							<span class="visits">
								{place.visits === 1 ? 'Visited' : `Visited ${place.visits}×`}{place.last_visited_at
									? `, last on ${day(place.last_visited_at)}`
									: ''}
								{#if place.verified}<span class="verified"> · checked in on the spot</span>{/if}
							</span>
						{/if}
						<span class="links">
							<a href={directions(place.lat, place.lng)} target="_blank" rel="noopener">Directions</a>
							{#if place.url}<a href={place.url} target="_blank" rel="noopener">Website</a>{/if}
						</span>
					</div>
					{#if waiting}
						<span class="check done" role="status"><Check size={16} strokeWidth={3} /> Today</span>
					{:else}
						<button
							class="check"
							class:again={visited}
							disabled={busy === place.id}
							onclick={() => run(place, () => checkIn(place))}
						>
							{#if visited}<Repeat size={15} strokeWidth={3} /> Again{:else}Check in{/if}
						</button>
					{/if}
				</div>
				{#if waiting}
					{@const options = dishesOf(place.countries)}
					<div class="ate">
						{#if options.length}
							<span class="muted">What did you have?</span>
							<div class="chips" role="group" aria-label="Dish eaten at {place.name}">
								{#each options as dish (dish.id)}
									<button
										aria-pressed={place.last_dish === dish.id}
										disabled={busy === place.id}
										onclick={() => pick(place, dish)}
									>
										{#if place.last_dish === dish.id}<Check size={14} strokeWidth={3} />{/if}
										{dish.name}
										{#if !tried.has(dish.id)}<span class="xp">+25</span>{/if}
									</button>
								{/each}
							</div>
						{/if}
						<button class="undo" disabled={busy === place.id} onclick={() => run(place, () => undoCheckIn(place))}>
							Undo check-in
						</button>
					</div>
				{/if}
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

	.places > li {
		padding: 14px 0;
		border-bottom: 1px solid var(--line);
	}

	.row {
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.passport {
		margin-top: 14px;
	}

	.passport h3 {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 14px;
		font-weight: 800;
	}

	.hint {
		font-size: 13px;
		margin: 6px 0 0;
	}

	.chips {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
		margin: 8px 0 0;
		padding: 0;
		list-style: none;
	}

	.chips > li,
	.chips > button {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		padding: 5px 11px;
		border-radius: 999px;
		border: 2px solid var(--line);
		font-size: 13px;
		font-weight: 700;
	}

	.chips > li.tried,
	.chips > button[aria-pressed='true'] {
		border-color: var(--completed);
		background: color-mix(in srgb, var(--completed) 16%, transparent);
	}

	.chips > button:hover {
		border-color: var(--accent);
	}

	.xp {
		color: var(--discovered);
		font-weight: 900;
		font-size: 12px;
	}

	.ate {
		margin-top: 10px;
		padding: 10px 12px;
		border-radius: 12px;
		background: var(--panel-raised);
		font-size: 13px;
	}

	.ate .chips {
		margin: 6px 0 4px;
	}

	.undo {
		margin-top: 4px;
		color: var(--muted);
		font-size: 13px;
		font-weight: 700;
		text-decoration: underline;
	}

	.visits {
		color: var(--text);
		font-weight: 700;
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
		cursor: default;
	}

	.check.again {
		--face: var(--discovered);
		color: var(--on-completed);
	}

	.check:disabled {
		opacity: 0.6;
	}
</style>
