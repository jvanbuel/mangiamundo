<script lang="ts">
	import Medal from './Medal.svelte';
	import type { Badge, EventProgress } from './types';

	let { badges, events }: { badges: Badge[]; events: EventProgress[] } = $props();

	const sorted = $derived([...badges].sort((a, b) => Number(b.earned) - Number(a.earned)));
	const running = $derived(events.filter((e) => e.status !== 'ended' && !e.earned));
	const kept = $derived(events.filter((e) => e.earned));

	const parse = (day: string) => new Date(`${day}T12:00:00Z`);
	const short = (day: string) =>
		parse(day).toLocaleDateString(undefined, { day: 'numeric', month: 'short', timeZone: 'UTC' });

	function timing(e: EventProgress): string {
		if (e.status === 'upcoming') return `Starts ${short(e.starts)}`;
		const today = parse(new Date().toISOString().slice(0, 10)).getTime();
		const left = Math.round((parse(e.ends).getTime() - today) / 86_400_000);
		return left <= 0 ? 'Last day' : `${left + 1} days left`;
	}
</script>

{#if running.length}
	<section class="limited">
		<h3>Limited time</h3>
		<ul>
			{#each running as e (e.id)}
				<li>
					<Medal kind="event" earned={e.status === 'active'} size={52} />
					<div class="info">
						<div class="row">
							<strong>{e.title}</strong>
							<span class="when">{timing(e)}</span>
						</div>
						<p class="muted">{e.description}</p>
						<div class="row progress">
							<div class="bar"><span style:width="{(e.progress / e.target) * 100}%"></span></div>
							<span class="muted">{e.progress} / {e.target}</span>
						</div>
						{#if e.places_nearby === 0}
							<p class="muted">No places nearby for this one yet.</p>
						{/if}
					</div>
				</li>
			{/each}
		</ul>
	</section>
{/if}

<ul class="grid">
	{#each kept as e (e.id)}
		<li title={e.description}>
			<Medal kind="event" />
			<strong>{e.title}</strong>
			<span class="muted">{short(e.starts)} – {short(e.ends)}</span>
		</li>
	{/each}
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
	.limited {
		margin: 4px 0 20px;
	}

	h3 {
		font-size: var(--text-base);
		margin-bottom: 8px;
	}

	.limited ul {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: 12px;
	}

	.limited li {
		display: flex;
		gap: 12px;
		align-items: flex-start;
	}

	.info {
		flex: 1;
		min-width: 0;
		display: grid;
		gap: 4px;
	}

	.row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
	}

	.info strong {
		font-weight: 700;
	}

	.when {
		flex: none;
		padding: 2px 8px;
		border-radius: 999px;
		background: color-mix(in srgb, var(--event) 16%, transparent);
		color: var(--event);
		font-size: var(--text-xs);
		font-weight: 700;
	}

	.info p {
		margin: 0;
		font-size: var(--text-sm);
	}

	.progress .bar {
		flex: 1;
	}

	.progress .bar span {
		background: var(--event);
	}

	.progress .muted {
		font-size: var(--text-sm);
		font-variant-numeric: tabular-nums;
	}

	.grid {
		list-style: none;
		margin: 0;
		padding: 4px 0 0;
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(104px, 1fr));
		gap: 20px 8px;
	}

	.grid li {
		display: grid;
		justify-items: center;
		align-content: start;
		gap: 4px;
		text-align: center;
	}

	.grid strong {
		margin-top: 6px;
		font-size: var(--text-sm);
		font-weight: 700;
		line-height: 1.2;
	}

	.grid .muted {
		font-size: var(--text-sm);
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
