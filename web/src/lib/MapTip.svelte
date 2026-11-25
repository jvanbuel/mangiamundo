<script lang="ts">
	import type { Country, CountryProgress } from './types';

	let { hover, info }: {
		hover: { x: number; y: number; name: string };
		info: { country: Country | undefined; progress: CountryProgress | undefined };
	} = $props();
</script>

<div class="tip" style:left="{hover.x}px" style:top="{hover.y}px">
	<strong>{info.country?.flag ?? ''} {hover.name}</strong>
	{#if info.progress}
		<span>{info.progress.visited} of {info.progress.places} {info.progress.places === 1 ? 'place' : 'places'} visited</span>
	{:else}
		<span>Not found in this area</span>
	{/if}
</div>

<style>
	.tip {
		position: absolute;
		z-index: 2;
		transform: translate(14px, 14px);
		pointer-events: none;
		display: grid;
		padding: 8px 10px;
		border-radius: 10px;
		background: var(--panel-raised);
		border: 1px solid var(--line);
		box-shadow: 0 8px 24px var(--shadow);
		font-size: var(--text-sm);
		white-space: nowrap;
	}

	.tip span {
		color: var(--muted);
	}
</style>
