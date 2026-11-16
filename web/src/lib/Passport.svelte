<script lang="ts">
	import KeyRound from '@lucide/svelte/icons/key-round';
	import Share2 from '@lucide/svelte/icons/share-2';
	import { app } from './state.svelte';
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
		<div><dt>Check-ins</dt><dd>{profile.visits}</dd></div>
		<div><dt>Signature dishes</dt><dd>{profile.dishes}</dd></div>
	</dl>

	<div class="actions">
		<button class="share" onclick={() => (app.dialog = 'share')}><Share2 size={16} strokeWidth={2.5} /> Share my map</button>
		<button onclick={() => (app.dialog = 'backup')}><KeyRound size={16} strokeWidth={2.5} /> Back up</button>
	</div>
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
		padding-top: 12px;
		border-top: 1px solid var(--line);
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

	.actions {
		display: flex;
		gap: 8px;
	}

	.actions button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: 6px;
		padding: 8px 12px 10px;
		border-radius: 12px;
		border: 2px solid var(--line);
		font-size: 14px;
		font-weight: 800;
	}

	.actions .share {
		--face: var(--accent);
		flex: 1;
		border: 0;
		background: var(--face);
		color: var(--on-accent);
		box-shadow: inset 0 -3px 0 color-mix(in srgb, var(--face) 70%, #000);
	}
</style>
