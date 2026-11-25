<script lang="ts">
	import { app } from './state.svelte';
	import type { Area } from './types';

	/** Switches to the city or the postal code found at the GPS position. */
	let { onpick }: { onpick: (area: Area) => void } = $props();
</script>

{#if app.location && app.location.options.length > 1}
	<div class="options" role="group" aria-label="Area around you">
		{#each app.location.options as option (option.id)}
			<button
				class:on={app.location.area.id === option.id}
				aria-pressed={app.location.area.id === option.id}
				onclick={() => onpick(option)}
			>
				<span>{option.kind === 'city' ? `All of ${option.name}` : `Only ${option.name}`}</span>
			</button>
		{/each}
	</div>
{/if}

<style>
	.options {
		display: flex;
		border: 1px solid var(--line);
		border-radius: 10px;
		overflow: hidden;
	}

	.options button {
		flex: 1;
		padding: 8px 10px;
		font-size: var(--text-sm);
	}

	.options button + button {
		border-left: 1px solid var(--line);
	}

	.options button.on {
		background: var(--panel);
		color: var(--accent);
		font-weight: 700;
	}
</style>
