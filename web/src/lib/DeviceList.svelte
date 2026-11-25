<script lang="ts">
	import type { Account } from './types';

	let { devices, busy, ago, onsignout }: {
		devices: Account['devices'];
		busy: boolean;
		ago: (at: number) => string;
		onsignout: (id: string) => void;
	} = $props();
</script>

<div class="group">
	<h3>Signed-in devices</h3>
	<ul>
		{#each devices as device (device.id)}
			<li>
				<span>
					<strong>{device.name ?? 'Unnamed device'}</strong>
					<span class="note">{device.current ? 'This device' : `Used ${ago(device.last_used_at)}`}</span>
				</span>
				{#if !device.current}
					<button class="secondary" disabled={busy} onclick={() => onsignout(device.id)}>Sign out</button>
				{/if}
			</li>
		{/each}
	</ul>
</div>

<style>
	ul {
		list-style: none;
		margin: 0;
		padding: 0;
		width: 100%;
	}

	li {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		padding: 6px 0;
	}

	li > span {
		display: flex;
		flex-direction: column;
	}
</style>
