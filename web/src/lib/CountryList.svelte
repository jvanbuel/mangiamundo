<script lang="ts">
	import { country } from './state.svelte';
	import { statusOf, type Discover } from './types';

	let { discover, onselect }: { discover: Discover; onselect: (code: string) => void } = $props();

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
						<span class="dot {entry.status}" title={entry.status}></span>
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
		font-family: inherit;
		font-size: 12px;
		text-transform: uppercase;
		letter-spacing: 0.08em;
		color: var(--muted);
		padding: 0 8px 6px;
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	button {
		width: 100%;
		display: grid;
		grid-template-columns: 26px 1fr auto 10px;
		align-items: center;
		gap: 10px;
		padding: 8px;
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
		font-size: 13px;
		font-variant-numeric: tabular-nums;
	}

	.dot {
		width: 10px;
		height: 10px;
		border-radius: 50%;
		background: var(--available);
	}

	.dot.discovered {
		background: var(--discovered);
	}

	.dot.completed {
		background: var(--completed);
	}
</style>
