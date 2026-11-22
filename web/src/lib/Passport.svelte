<script lang="ts">
	import Share from '@lucide/svelte/icons/share-2';
	import { shareProgress } from './share';
	import type { Discover } from './types';

	let { discover, place }: { discover: Discover; place: string } = $props();
	let sharing = $state(false);

	async function share() {
		sharing = true;
		await shareProgress(discover, place);
		sharing = false;
	}

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

	<button class="share" disabled={sharing} onclick={share}><Share size={15} strokeWidth={2.5} /> Share my map</button>
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

	.share {
		justify-self: start;
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 6px 12px;
		border-radius: 9px;
		background: var(--accent);
		color: var(--on-accent);
		font-weight: 800;
		font-size: var(--text-sm);
		box-shadow: 0 2px 0 color-mix(in srgb, var(--accent) 70%, black);
	}

	.share:active {
		transform: translateY(2px);
		box-shadow: none;
	}

	.share:disabled {
		opacity: 0.6;
	}

	dd .muted {
		font-weight: 500;
	}
</style>
