<script lang="ts">
	import type { Place } from './types';

	let { hours }: { hours: NonNullable<Place['hours']> } = $props();

	/** Always open, with no exceptions: one line says it better than seven. A week that is open
	 * around the clock may still have a closed holiday coming, so go by the rule itself. */
	const allDay = $derived(hours.raw.trim() === '24/7');
</script>

<section>
	<h3>Opening hours</h3>
	{#if allDay}
		<p class="status open">Open 24 hours, every day</p>
	{:else if hours.status}
		<p class="status" class:open={hours.status.open}>{hours.status.text}</p>
	{/if}
	{#if allDay}
		<!-- The line above says it all. -->
	{:else if hours.week}
		<table>
			<tbody>
				{#each hours.week as d (d.name)}
					<tr class:today={d.today}>
						<th scope="row">{d.name}{#if d.today}<span class="sr-only"> (today)</span>{/if}</th>
						<td>{d.open.length ? d.open.join(', ') : 'Closed'}</td>
					</tr>
				{/each}
			</tbody>
		</table>
	{:else}
		<p class="muted raw">{hours.raw}</p>
	{/if}
</section>

<style>
	h3 {
		font-size: var(--text-base);
		margin: 0 0 6px;
	}

	p {
		margin: 2px 0 0;
		font-size: var(--text-sm);
	}

	section {
		margin-top: 18px;
		font-size: var(--text-sm);
	}

	.status {
		color: var(--muted);
		font-weight: 700;
		margin-bottom: 6px;
	}

	.status.open {
		color: var(--text);
	}

	table {
		border-collapse: collapse;
	}

	th,
	td {
		padding: 3px 16px 3px 0;
		text-align: left;
		vertical-align: top;
		font-weight: 500;
	}

	tr.today th,
	tr.today td {
		font-weight: 700;
	}

	.sr-only {
		position: absolute;
		width: 1px;
		height: 1px;
		overflow: hidden;
		clip-path: inset(50%);
		white-space: nowrap;
	}

	.raw {
		font-family: ui-monospace, monospace;
		font-size: var(--text-sm);
		overflow-wrap: anywhere;
	}
</style>
