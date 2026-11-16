<script lang="ts">
	import Medal from './Medal.svelte';
	import type { Badge } from './types';

	let { badges }: { badges: Badge[] } = $props();

	const sorted = $derived([...badges].sort((a, b) => Number(b.earned) - Number(a.earned)));
</script>

<ul>
	{#each sorted as badge (badge.id)}
		<li title={badge.description}>
			<Medal kind={badge.kind} earned={badge.earned} />
			<strong>{badge.title}</strong>
			<span class="muted">
				{#if badge.earned}Earned{:else}{badge.progress} / {badge.target}{/if}
			</span>
			<span class="sr-only">{badge.description}</span>
		</li>
	{/each}
</ul>

<style>
	ul {
		list-style: none;
		margin: 0;
		padding: 4px 0 0;
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(104px, 1fr));
		gap: 20px 8px;
	}

	li {
		display: grid;
		justify-items: center;
		align-content: start;
		gap: 4px;
		text-align: center;
	}

	strong {
		margin-top: 6px;
		font-size: 14px;
		font-weight: 800;
		line-height: 1.2;
	}

	.muted {
		font-size: 13px;
		font-variant-numeric: tabular-nums;
	}

	.sr-only {
		position: absolute;
		width: 1px;
		height: 1px;
		overflow: hidden;
		clip-path: inset(50%);
	}
</style>
