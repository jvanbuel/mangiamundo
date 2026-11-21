<script lang="ts">
	import X from '@lucide/svelte/icons/x';
	import { fly } from 'svelte/transition';
	import Medal from './Medal.svelte';
	import { app } from './state.svelte';

	// Svelte animates in script, which the stylesheet's reduced-motion rule can't reach:
	// with reduced motion a toast fades instead of sliding.
	const still = matchMedia('(prefers-reduced-motion: reduce)').matches;
	const dismiss = (id: number) => (app.toasts = app.toasts.filter((t) => t.id !== id));
</script>

<div class="toasts" aria-live="polite">
	{#each app.toasts as toast (toast.id)}
		<div class="toast" transition:fly={{ y: still ? 0 : -16, duration: 250 }}>
			{#if toast.icon && 'badge' in toast.icon}
				<Medal kind={toast.icon.badge} size={40} />
			{:else if toast.icon}
				<span class="icon" aria-hidden="true">{toast.icon.text}</span>
			{/if}
			<div class="body">
				<strong>{toast.title}</strong>
				<p>{toast.text}</p>
			</div>
			<button class="close" aria-label="Dismiss" onclick={() => dismiss(toast.id)}>
				<X size={16} strokeWidth={2.5} aria-hidden="true" />
			</button>
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

	/* Over the map, beside the panel, rather than across both. */
	@media (min-width: 761px) {
		.toasts {
			left: calc(400px + (100vw - 400px) / 2);
		}
	}

	.toast {
		pointer-events: auto;
	}

	.body {
		flex: 1;
		min-width: 0;
	}

	.close {
		align-self: flex-start;
		display: grid;
		place-items: center;
		margin: -12px -14px -12px 0;
		padding: 14px;
		color: var(--muted);
	}

	.close:hover {
		color: var(--text);
	}

	.toast {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 12px 14px;
		border-radius: 16px;
		background: var(--panel);
		box-shadow: 0 2px 6px var(--shadow), 0 12px 32px var(--shadow);
	}

	.icon {
		min-width: 40px;
		text-align: center;
		font-size: 28px;
		font-weight: 900;
		color: var(--accent);
	}

	strong {
		font-weight: 700;
	}

	p {
		margin: 0;
		font-size: var(--text-sm);
		color: var(--muted);
	}
</style>
