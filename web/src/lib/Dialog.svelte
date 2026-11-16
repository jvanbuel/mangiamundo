<script lang="ts">
	import X from '@lucide/svelte/icons/x';
	import type { Snippet } from 'svelte';

	let { title, onclose, children }: { title: string; onclose: () => void; children: Snippet } = $props();

	let dialog: HTMLDialogElement;

	$effect(() => {
		dialog.showModal();
	});
</script>

<!-- A native modal: focus is trapped, Escape closes it, and clicking the backdrop does too. -->
<dialog
	bind:this={dialog}
	aria-label={title}
	onclose={onclose}
	onclick={(e) => {
		if (e.target === dialog) dialog.close();
	}}
>
	<div class="body">
		<header>
			<h2>{title}</h2>
			<button class="close" aria-label="Close" onclick={() => dialog.close()}><X size={20} strokeWidth={2.5} /></button>
		</header>
		{@render children()}
	</div>
</dialog>

<style>
	dialog {
		width: min(440px, calc(100vw - 32px));
		max-height: calc(100dvh - 32px);
		padding: 0;
		border-radius: 20px;
		border: 2px solid var(--line);
		border-bottom-width: 5px;
		background: var(--panel);
		color: var(--text);
		box-shadow: 0 24px 64px var(--shadow);
	}

	dialog::backdrop {
		background: rgb(8 18 30 / 0.5);
	}

	.body {
		padding: 18px 18px 20px;
	}

	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		margin-bottom: 12px;
	}

	h2 {
		font-size: 20px;
	}

	.close {
		display: grid;
		place-items: center;
		width: 36px;
		height: 36px;
		border-radius: 50%;
		color: var(--muted);
	}

	.close:hover {
		background: var(--panel-raised);
		color: var(--text);
	}
</style>
