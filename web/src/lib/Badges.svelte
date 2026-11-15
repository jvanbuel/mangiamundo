<script lang="ts">
	import type { Badge } from './types';

	let { badges }: { badges: Badge[] } = $props();

	const sorted = $derived([...badges].sort((a, b) => Number(b.earned) - Number(a.earned)));
</script>

<ul>
	{#each sorted as badge (badge.id)}
		<li class:earned={badge.earned}>
			<span class="icon" aria-hidden="true">{badge.icon}</span>
			<div>
				<strong>{badge.title}</strong>
				<p class="muted">{badge.description}</p>
				{#if !badge.earned}
					<div class="progress">
						<div class="bar"><span style:width="{(badge.progress / badge.target) * 100}%"></span></div>
						<span class="muted">{badge.progress}/{badge.target}</span>
					</div>
				{/if}
			</div>
			{#if badge.earned}<span class="tick" aria-label="Earned">✓</span>{/if}
		</li>
	{/each}
</ul>

<style>
	ul {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: 8px;
	}

	li {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 12px;
		border-radius: 12px;
		background: var(--panel-raised);
	}

	li > div {
		flex: 1;
		min-width: 0;
	}

	.icon {
		font-size: 26px;
		filter: grayscale(1);
		opacity: 0.55;
	}

	li.earned .icon {
		filter: none;
		opacity: 1;
	}

	p {
		margin: 0;
		font-size: 13px;
	}

	.progress {
		display: flex;
		align-items: center;
		gap: 8px;
		margin-top: 6px;
		font-size: 12px;
	}

	.progress .bar {
		flex: 1;
	}

	.tick {
		color: var(--completed);
		font-weight: 700;
	}
</style>
