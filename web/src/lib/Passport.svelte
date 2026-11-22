<script lang="ts">
	import type { Discover } from './types';

	let { discover }: { discover: Discover } = $props();

	const profile = $derived(discover.profile);
	const pct = (part: number, whole: number) => (whole ? (part / whole) * 100 : 0);
</script>

<section class="passport">
	{#if discover.places_visited === 0}
		<!-- All zeros say nothing yet: one line on how to start instead. -->
		<div class="start">
			<strong>Level {profile.level}</strong>
			<p class="muted">Check in at a restaurant to taste the first of your {discover.world.available} countries.</p>
		</div>
	{:else}
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
		<div><dt>Diamond countries</dt><dd>{discover.world.completed}</dd></div>
	</dl>
	{/if}
</section>

<style>
	.passport {
		display: grid;
		gap: 14px;
	}

	.start p {
		margin: 2px 0 0;
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
		padding-top: 12px;
		border-top: 1px solid var(--line);
	}

	dt {
		color: var(--muted);
		font-size: var(--text-sm);
	}

	dd {
		margin: 2px 0 0;
		font-weight: 700;
		font-variant-numeric: tabular-nums;
	}

	dd .muted {
		font-weight: 500;
	}
</style>
