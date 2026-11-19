<script lang="ts">
	import type { AreaProgress } from './types';

	let { progress }: { progress: AreaProgress } = $props();

	const R = 34;
	const CIRCUMFERENCE = 2 * Math.PI * R;
</script>

<div class="overlay">
	<div
		class="panel"
		role="progressbar"
		aria-label="Loading restaurants"
		aria-valuemin={0}
		aria-valuemax={100}
		aria-valuenow={progress.percent}
		aria-valuetext="{progress.text}, {progress.percent}%"
	>
		<svg viewBox="0 0 80 80" aria-hidden="true">
			<circle class="track" cx="40" cy="40" r={R} />
			<circle class="fill" cx="40" cy="40" r={R} stroke-dasharray={CIRCUMFERENCE} stroke-dashoffset={CIRCUMFERENCE * (1 - progress.percent / 100)} />
			<text x="40" y="40">{progress.percent}%</text>
		</svg>
		<p>{progress.text}…</p>
	</div>
</div>

<style>
	.overlay {
		position: fixed;
		inset: 0;
		display: grid;
		place-items: center;
		pointer-events: none;
		z-index: 20;
	}

	.panel {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 8px;
		padding: 20px 24px 16px;
		border-radius: 16px;
		border: 2px solid var(--line);
		border-bottom-width: 4px;
		background: var(--panel);
	}

	svg {
		width: 96px;
		height: 96px;
	}

	circle {
		fill: none;
		stroke-width: 8;
	}

	.track {
		stroke: var(--line);
	}

	.fill {
		stroke: var(--accent);
		stroke-linecap: round;
		/* Start at the top and fill clockwise. */
		transform: rotate(-90deg);
		transform-origin: center;
		transition: stroke-dashoffset 0.4s linear;
	}

	text {
		fill: var(--text);
		font-size: 18px;
		font-weight: 800;
		text-anchor: middle;
		dominant-baseline: central;
	}

	p {
		margin: 0;
		font-size: 14px;
		font-weight: 700;
		color: var(--muted);
	}

	@media (prefers-reduced-motion: reduce) {
		.fill {
			transition: none;
		}
	}
</style>
