<script lang="ts">
	import type { Discover } from './types';

	let { discover }: { discover: Discover } = $props();

	const profile = $derived(discover.profile);
	const levelPct = $derived(
		((profile.xp - profile.level_xp) / (profile.next_level_xp - profile.level_xp)) * 100
	);
	const pct = (part: number, whole: number) => (whole ? (part / whole) * 100 : 0);
</script>

<section class="passport">
	<div class="level">
		<div class="badge" aria-hidden="true">{profile.level}</div>
		<div class="grow">
			<div class="row">
				<strong>Level {profile.level}</strong>
				<span class="muted">{profile.xp} / {profile.next_level_xp} XP</span>
			</div>
			<div class="bar"><span style:width="{levelPct}%"></span></div>
		</div>
	</div>

	<div class="world">
		<div class="row">
			<h2>The world in your city</h2>
		</div>
		<p class="big">
			<span>{discover.world.discovered}</span> of {discover.world.available} countries tasted
		</p>
		<div class="bar tall">
			<span style:width="{pct(discover.world.discovered, discover.world.available)}%"></span>
		</div>
		<p class="muted small">
			{discover.places_visited} of {discover.places} places visited · {discover.world.completed}
			{discover.world.completed === 1 ? 'country' : 'countries'} completed
		</p>
	</div>

	<ul class="continents">
		{#each discover.continents as continent (continent.code)}
			<li class:empty={continent.available === 0}>
				<span class="name">{continent.name}</span>
				<div class="bar"><span style:width="{pct(continent.discovered, continent.available)}%"></span></div>
				<span class="count">
					{#if continent.available === 0}—{:else}{continent.discovered}/{continent.available}{/if}
				</span>
			</li>
		{/each}
	</ul>
</section>

<style>
	.passport {
		display: grid;
		gap: 16px;
	}

	.level {
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.badge {
		width: 40px;
		height: 40px;
		flex: none;
		display: grid;
		place-items: center;
		border-radius: 50%;
		border: 2px solid var(--accent);
		font-family: var(--display);
		font-size: 18px;
		font-weight: 700;
		color: var(--accent);
	}

	.grow {
		flex: 1;
		display: grid;
		gap: 6px;
	}

	.row {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		gap: 8px;
		font-size: 13px;
	}

	h2 {
		font-size: 13px;
		font-family: inherit;
		text-transform: uppercase;
		letter-spacing: 0.08em;
		color: var(--muted);
	}

	.big {
		margin: 4px 0 8px;
		font-family: var(--display);
		font-size: 19px;
	}

	.big span {
		font-size: 30px;
		font-weight: 700;
		color: var(--accent);
	}

	.tall {
		height: 8px;
		border-radius: 4px;
	}

	.small {
		margin: 8px 0 0;
		font-size: 13px;
	}

	.continents {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: 7px;
		font-size: 13px;
	}

	.continents li {
		display: grid;
		grid-template-columns: 108px 1fr 40px;
		align-items: center;
		gap: 10px;
	}

	.continents li.empty {
		color: var(--muted);
		opacity: 0.6;
	}

	.count {
		text-align: right;
		font-variant-numeric: tabular-nums;
		color: var(--muted);
	}
</style>
