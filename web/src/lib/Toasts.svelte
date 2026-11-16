<script lang="ts">
	import { fly } from 'svelte/transition';
	import Medal from './Medal.svelte';
	import { app } from './state.svelte';
</script>

<div class="toasts" aria-live="polite">
	{#each app.toasts as toast (toast.id)}
		<div class="toast" transition:fly={{ y: -16, duration: 250 }}>
			{#if toast.icon && 'badge' in toast.icon}
				<Medal kind={toast.icon.badge} size={40} />
			{:else if toast.icon}
				<span class="icon" aria-hidden="true">{toast.icon.text}</span>
			{/if}
			<div>
				<strong>{toast.title}</strong>
				<p>{toast.text}</p>
			</div>
		</div>
	{/each}
</div>

<style>
	.toasts {
		position: fixed;
		top: 16px;
		left: 50%;
		transform: translateX(-50%);
		z-index: 20;
		display: grid;
		gap: 8px;
		width: min(360px, calc(100vw - 32px));
		pointer-events: none;
	}

	.toast {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 12px 14px;
		border-radius: 16px;
		background: var(--panel);
		border: 2px solid var(--line);
		border-bottom-width: 4px;
		box-shadow: 0 12px 32px var(--shadow);
	}

	.icon {
		min-width: 40px;
		text-align: center;
		font-size: 28px;
		font-weight: 900;
		color: var(--accent);
	}

	strong {
		font-weight: 800;
	}

	p {
		margin: 0;
		font-size: 13px;
		color: var(--muted);
	}
</style>
