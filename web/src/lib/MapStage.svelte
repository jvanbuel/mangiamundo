<script lang="ts">
	import WorldMap from './WorldMap.svelte';
	import Dices from '@lucide/svelte/icons/dices';
	import { app, setProjection, spin } from './state.svelte';

	let { systemDark, onselect }: {
		/** Whether the system is in dark mode, so the map repaints when it changes. */
		systemDark: boolean;
		onselect: (code: string, name: string, available: boolean) => void;
	} = $props();
</script>

<div class="stage">
	{#if app.catalog}
		<WorldMap
			countries={app.catalog.countries}
			progress={app.discover?.countries ?? []}
			selected={app.selected}
			location={app.location}
			projection={app.projection}
			theme={`${app.theme}-${systemDark}`}
			spinning={app.spinning}
			{onselect}
		/>
	{/if}

	<div class="controls">
		{#if app.discover}
			<button class="spin" disabled={app.spinning} onclick={spin} title="Where shall we eat? Spin for a place to try">
				<Dices size={16} strokeWidth={2.5} aria-hidden="true" />{app.spinning ? 'Spinning…' : 'Spin'}
			</button>
		{/if}
		<div class="toggle" role="group" aria-label="Map projection">
			<button aria-pressed={app.projection === 'globe'} onclick={() => setProjection('globe')}>Globe</button>
			<button aria-pressed={app.projection === 'mercator'} onclick={() => setProjection('mercator')}>Flat</button>
		</div>
	</div>

	{#if app.location}<ul class="legend" aria-label="Legend">
		<li><span class="swatch available"></span>To discover</li>
		<li><span class="swatch discovered"></span>Gold</li>
		<li><span class="swatch platinum"></span>Platinum</li>
		<li><span class="swatch completed"></span>Diamond</li>
		<li><span class="swatch none"></span>None nearby</li>
	</ul>{/if}
</div>

<style>
	.stage {
		position: relative;
		grid-column: 2;
		grid-row: 1;
		min-height: 0;
		overflow: hidden;
	}

	.controls {
		position: absolute;
		top: 16px;
		right: 16px;
		z-index: 1;
		display: flex;
		gap: 8px;
	}

	.spin {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 6px 16px;
		border-radius: 999px;
		background: var(--accent);
		color: var(--on-accent);
		font-size: var(--text-sm);
		font-weight: 700;
		box-shadow: inset 0 -3px 0 color-mix(in srgb, var(--accent) 70%, #000);
	}

	.spin:disabled {
		opacity: 0.8;
	}

	.toggle {
		display: flex;
		padding: 3px;
		border-radius: 999px;
		background: var(--glass);
		border: 1px solid var(--line);
	}

	.toggle button {
		padding: 6px 16px;
		border-radius: 999px;
		font-size: var(--text-sm);
		font-weight: 700;
		color: var(--muted);
	}

	.toggle button[aria-pressed='true'] {
		background: var(--accent);
		color: var(--on-accent);
	}

	.legend {
		position: absolute;
		left: 16px;
		bottom: 16px;
		z-index: 1;
		list-style: none;
		margin: 0;
		padding: 8px 12px;
		display: flex;
		flex-wrap: wrap;
		gap: 4px 14px;
		border-radius: 10px;
		background: var(--glass);
		border: 1px solid var(--line);
		font-size: var(--text-xs);
		color: var(--muted);
	}

	.legend li {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	/* The same shapes as the list's status marks, so the states don't rest on colour alone. */
	.swatch {
		width: 12px;
		height: 12px;
		border-radius: 50%;
		border: 2px solid var(--available-edge);
		background: var(--available);
	}

	.swatch.discovered {
		border-color: var(--discovered);
		background: linear-gradient(to top, var(--discovered) 50%, transparent 50%);
	}

	.swatch.platinum {
		border-color: var(--platinum-edge);
		background: var(--platinum-metal);
	}

	.swatch.completed {
		border-color: var(--completed-edge);
		background: var(--diamond);
	}

	.swatch.none {
		border-radius: 3px;
		border: 1px solid rgb(0 0 0 / 0.15);
		background: var(--land);
	}

	@media (max-width: 760px) {
		.stage {
			grid-column: 1;
			grid-row: 1;
		}

		.legend {
			left: 8px;
			bottom: 8px;
			padding: 6px 10px;
			gap: 2px 10px;
		}

		.controls {
			top: 8px;
			right: 8px;
		}
	}
</style>
