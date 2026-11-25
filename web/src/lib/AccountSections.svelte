<script lang="ts">
	import type { Snippet } from 'svelte';

	let { children }: { children: Snippet } = $props();
</script>

<section class="account">
	{@render children()}
</section>

<style>
	/* One look for every section of the settings, SignIn's included. */
	.account {
		margin-top: 4px;
	}

	.account :global(.group) {
		display: grid;
		gap: 8px;
		justify-items: start;
		padding: 14px 0;
		border-top: 1px solid var(--line);
	}

	.account :global(.group > *) {
		max-width: 100%;
	}

	.account :global(h3) {
		margin: 0;
		font-size: var(--text-sm);
		font-weight: 800;
	}

	.account :global(.note) {
		margin: 0;
		font-size: var(--text-xs);
		line-height: 1.45;
		color: var(--muted);
	}

	.account :global(.action),
	.account :global(.secondary) {
		padding: 8px 14px;
		border-radius: 10px;
		font-weight: 700;
		font-size: var(--text-sm);
		white-space: nowrap;
	}

	/* The one main action of a form. */
	.account :global(.action) {
		background: var(--accent);
		color: var(--on-accent);
	}

	.account :global(.secondary) {
		background: var(--panel);
		color: var(--text);
		border: 1px solid var(--line);
	}

	.account :global(.action:disabled),
	.account :global(.secondary:disabled) {
		opacity: 0.5;
	}

	.account :global(form) {
		display: grid;
		gap: 6px;
		width: 100%;
	}

	.account :global(label) {
		font-size: var(--text-xs);
		font-weight: 700;
		color: var(--muted);
	}

	.account :global(.row) {
		display: flex;
		gap: 8px;
	}

	.account :global(input) {
		flex: 1;
		min-width: 0;
		padding: 8px 10px;
		border-radius: 10px;
		border: 1px solid var(--line);
		background: var(--panel);
		color: var(--text);
		font: inherit;
		font-size: var(--text-sm);
	}

	/* Codes are typed in capitals; addresses aren't. */
	.account :global(.code-input) {
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}
</style>
