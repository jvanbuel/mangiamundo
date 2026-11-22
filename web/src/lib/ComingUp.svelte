<script lang="ts">
	import Bell from '@lucide/svelte/icons/bell';
	import BellRing from '@lucide/svelte/icons/bell-ring';
	import { app, country, select, toggleReminders } from './state.svelte';
	import type { Celebration } from './types';

	let { celebrations }: { celebrations: Celebration[] } = $props();

	// One by default, so the list of countries stays in view.
	let expanded = $state(false);
	const shown = $derived(celebrations.slice(0, expanded ? 4 : 1));
	const more = $derived(Math.min(celebrations.length, 4) - 1);
	const today = new Date().toISOString().slice(0, 10);

	const parse = (day: string) => new Date(`${day}T12:00:00Z`);
	const month = (day: string) => parse(day).toLocaleDateString(undefined, { month: 'short', timeZone: 'UTC' });
	const dayNumber = (day: string) => parse(day).getUTCDate();

	function when(c: Celebration): string {
		if (c.starts <= today) return c.ends > today ? 'On now' : 'Today';
		const days = Math.round((parse(c.starts).getTime() - parse(today).getTime()) / 86_400_000);
		return days === 1 ? 'Tomorrow' : `In ${days} days`;
	}

	function nearbyText(c: Celebration): string {
		const total = c.nearby.reduce((sum, n) => sum + n.places, 0);
		const flags = c.nearby.map((n) => country(n.code)?.flag ?? '').join(' ');
		return `${flags} ${total} ${total === 1 ? 'place' : 'places'} nearby`;
	}
</script>

<section>
	<header>
		<h2>Coming up</h2>
		{#if app.reminders === 'on' || app.reminders === 'off'}
			<button
				class="remind"
				class:on={app.reminders === 'on'}
				aria-pressed={app.reminders === 'on'}
				disabled={app.remindersBusy}
				onclick={toggleReminders}
			>
				{#if app.reminders === 'on'}<BellRing size={16} strokeWidth={2.5} /> Reminders on{:else}<Bell
						size={16}
						strokeWidth={2.5}
					/> Remind me{/if}
			</button>
		{/if}
	</header>
	{#if app.reminders === 'install'}
		<p class="muted note">To get reminders on iPhone, add Mangiamundo to your home screen first.</p>
	{/if}

	<ul>
		{#each shown as c (c.id + c.starts)}
			<li>
				<button onclick={() => select(c.nearby[0].code)}>
					<span class="date" aria-hidden="true">
						<span class="month">{month(c.starts)}</span>
						<span class="day">{dayNumber(c.starts)}</span>
					</span>
					<span class="text">
						<span class="title">{c.title}</span>
						<span class="muted">{when(c)}{c.approximate ? ' (date may shift)' : ''} · {nearbyText(c)}</span>
					</span>
				</button>
			</li>
		{/each}
	</ul>
	{#if more > 0}
		<button class="more" aria-expanded={expanded} onclick={() => (expanded = !expanded)}>
			{expanded ? 'Show less' : `${more} more coming up`}
		</button>
	{/if}
</section>

<style>
	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		margin-bottom: 6px;
	}

	h2 {
		font-size: var(--text-lg);
	}

	.remind {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 6px 12px;
		border-radius: 999px;
		border: 2px solid var(--line);
		font-size: var(--text-sm);
		font-weight: 700;
		color: var(--accent);
	}

	.remind.on {
		border-color: transparent;
		background: color-mix(in srgb, var(--accent) 14%, transparent);
	}

	.note {
		margin: 0 0 8px;
		font-size: var(--text-sm);
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	li + li {
		border-top: 1px solid var(--line);
	}

	li button {
		width: 100%;
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 10px 0;
		text-align: left;
	}

	/* A little tear-off calendar page. */
	.date {
		width: 46px;
		flex: none;
		display: grid;
		border-radius: 10px;
		overflow: hidden;
		border: 2px solid var(--line);
		border-bottom-width: 3px;
		background: var(--panel);
		text-align: center;
		line-height: 1.1;
	}

	.month {
		padding: 2px 0;
		background: var(--event);
		color: var(--on-event);
		font-size: var(--text-xs);
		font-weight: 700;
		text-transform: uppercase;
	}

	.day {
		padding: 3px 0 4px;
		font-size: var(--text-lg);
		font-weight: 900;
	}

	.more {
		color: var(--accent);
		font-size: var(--text-sm);
		font-weight: 700;
		padding: 4px 0;
	}

	.text {
		display: grid;
		min-width: 0;
	}

	.title {
		font-weight: 700;
	}

	.text .muted {
		font-size: var(--text-sm);
	}
</style>
