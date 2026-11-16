<script lang="ts">
	import type { Discover } from './types';

	let { discover }: { discover: Discover } = $props();

	const profile = $derived(discover.profile);
	const pct = (part: number, whole: number) => (whole ? (part / whole) * 100 : 0);
</script>

<section class="passport">
	<div class="stat">
		<div class="row">
			<span>Level {profile.level}</span>
			<span class="muted">{profile.xp} / {profile.next_level_xp} XP</span>
		</div>
		<div class="bar">
			<span style:width="{pct(profile.xp - profile.level_xp, profile.next_level_xp - profile.level_xp)}%"></span>
		</div>
	</div>

	<div class="stat">
		<div class="row">
			<span>Countries tasted</span>
			<span><strong>{discover.world.discovered}</strong> <span class="muted">of {discover.world.available}</span></span>
		</div>
		<div class="bar"><span style:width="{pct(discover.world.discovered, discover.world.available)}%"></span></div>
	</div>

	<dl class="totals">
		<div><dt>Places visited</dt><dd>{discover.places_visited} <span class="muted">of {discover.places}</span></dd></div>
		<div><dt>Countries completed</dt><dd>{discover.world.completed}</dd></div>
	</dl>

	<ul class="continents" aria-label="Countries tasted per continent">
		{#each discover.continents as continent (continent.code)}
			<li class:empty={continent.available === 0}>
				<span>{continent.name}</span>
				<div class="bar"><span style:width="{pct(continent.discovered, continent.available)}%"></span></div>
				<span class="count">
					{#if continent.available === 0}–{:else}{continent.discovered}/{continent.available}{/if}
				</span>
			</li>
		{/each}
	</ul>
</section>

<style>
	.passport {
		display: grid;
		gap: 14px;
	}

	.stat {
		display: grid;
		gap: 6px;
	}

	.row {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		gap: 8px;
		font-weight: 700;
	}

	.row .muted {
		font-weight: 500;
	}

	.totals {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 8px;
		margin: 0;
		padding: 12px 0;
		border-block: 1px solid var(--line);
	}

	dt {
		color: var(--muted);
		font-size: 13px;
	}

	dd {
		margin: 2px 0 0;
		font-weight: 700;
		font-variant-numeric: tabular-nums;
	}

	dd .muted {
		font-weight: 500;
	}

	.continents {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: 7px;
		font-size: 14px;
	}

	.continents li {
		display: grid;
		grid-template-columns: 112px 1fr 36px;
		align-items: center;
		gap: 10px;
	}

	.continents li.empty {
		color: var(--muted);
	}

	.count {
		text-align: right;
		font-variant-numeric: tabular-nums;
		color: var(--muted);
	}
</style>
