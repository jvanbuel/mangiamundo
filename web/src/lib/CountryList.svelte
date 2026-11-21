<script lang="ts">
	import Check from '@lucide/svelte/icons/check';
	import { country } from './state.svelte';
	import { statusOf, type Discover } from './types';

	let { discover, onselect }: { discover: Discover; onselect: (code: string) => void } = $props();

	const LABELS = { available: 'To discover', discovered: 'Tasted', completed: 'Completed' } as const;

	const groups = $derived(
		discover.continents
			.filter((continent) => continent.available > 0)
			.map((continent) => ({
				continent,
				countries: discover.countries
					.filter((c) => c.continent === continent.code)
					.map((c) => ({ ...c, info: country(c.code), status: statusOf(c) }))
					.sort((a, b) => (a.info?.name ?? '').localeCompare(b.info?.name ?? ''))
			}))
	);
</script>

{#each groups as group (group.continent.code)}
	<section>
		<h3>
			{group.continent.name}
			<span>{group.continent.discovered}/{group.continent.available}</span>
		</h3>
		<ul>
			{#each group.countries as entry (entry.code)}
				<li>
					<button onclick={() => onselect(entry.code)}>
						<span class="flag" aria-hidden="true">{entry.info?.flag}</span>
						<span class="name">{entry.info?.name ?? entry.code}</span>
						<span class="count">{entry.visited}/{entry.places}</span>
						<span class="mark {entry.status}" title={LABELS[entry.status]}>
							{#if entry.status === 'completed'}<Check size={10} strokeWidth={4} aria-hidden="true" />{/if}
						</span>
					</button>
				</li>
			{/each}
		</ul>
	</section>
{/each}

<style>
	section + section {
		margin-top: 16px;
	}

	h3 {
		display: flex;
		justify-content: space-between;
		font-size: var(--text-base);
		font-weight: 700;
		letter-spacing: 0;
		padding: 0 8px 6px;
	}

	h3 span {
		font-weight: 500;
		font-size: var(--text-sm);
		color: var(--muted);
		font-variant-numeric: tabular-nums;
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	button {
		width: 100%;
		display: grid;
		grid-template-columns: 26px 1fr auto 14px;
		align-items: center;
		gap: 10px;
		padding: 10px 8px;
		border-radius: 10px;
		text-align: left;
	}

	button:hover {
		background: var(--panel-raised);
	}

	.flag {
		font-size: 20px;
		line-height: 1;
	}

	.count {
		color: var(--muted);
		font-size: var(--text-sm);
		font-variant-numeric: tabular-nums;
	}

	/* Status by shape as well as colour: empty ring, half full, full with a tick. */
	.mark {
		width: 14px;
		height: 14px;
		display: grid;
		place-items: center;
		border-radius: 50%;
		border: 2px solid var(--available-edge);
		background: var(--available);
	}

	.mark.discovered {
		border-color: var(--discovered);
		background: linear-gradient(to top, var(--discovered) 50%, transparent 50%);
	}

	.mark.completed {
		border-color: var(--completed);
		background: var(--completed);
		color: var(--on-completed);
	}
</style>
