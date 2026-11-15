<script lang="ts">
	import { fly } from 'svelte/transition';
	import { app } from './state.svelte';
</script>

<div class="toasts" aria-live="polite">
	{#each app.toasts as toast (toast.id)}
		<div class="toast" transition:fly={{ y: -16, duration: 250 }}>
			<span class="icon" aria-hidden="true">{toast.icon}</span>
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
		border-radius: 14px;
		background: var(--panel-raised);
		border: 1px solid var(--accent);
		box-shadow: 0 12px 32px var(--shadow);
	}

	.icon {
		font-size: 28px;
	}

	p {
		margin: 0;
		font-size: 13px;
		color: var(--muted);
	}
</style>
