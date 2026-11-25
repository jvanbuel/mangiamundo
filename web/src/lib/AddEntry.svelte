<script lang="ts">
	import AddPlace from './AddPlace.svelte';
	import { app, lookAgainNearby } from './state.svelte';

	let { adding = $bindable() }: { adding: boolean } = $props();
</script>

{#if adding}
	<div class="card"><AddPlace onclose={() => (adding = false)} /></div>
{:else}
	<p class="muted add-place">Is a restaurant missing? <button onclick={() => (adding = true)}>Add it</button></p>
	<p class="muted add-place">
		Just added it in an OpenStreetMap app?
		<button disabled={app.lookingNearby} onclick={lookAgainNearby}>{app.lookingNearby ? 'Looking…' : 'Look again'}</button>
	</p>
{/if}

<style>
	.add-place {
		margin-top: 20px;
		text-align: center;
	}

	.add-place button {
		color: inherit;
		text-decoration: underline;
	}

	.add-place + .add-place {
		margin-top: 4px;
	}

	.add-place button:disabled {
		opacity: 0.6;
	}

	/* Panels sit on a tinted surface. The heavy lower edge is kept for things you can press. */
	.card {
		padding: 16px;
		border-radius: 14px;
		background: var(--panel-raised);
		transition: opacity 0.2s;
	}
</style>
