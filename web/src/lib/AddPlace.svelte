<script lang="ts">
	import { addPlace, app } from './state.svelte';

	let { onclose }: { onclose: () => void } = $props();

	let name = $state('');
	let country = $state('');
	let address = $state('');
	let website = $state('');
	let busy = $state(false);

	// Only countries with a cuisine can be the food a place serves.
	const countries = $derived(
		(app.catalog?.countries ?? []).filter((c) => c.cuisine).sort((a, b) => a.name.localeCompare(b.name))
	);

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		const added = await addPlace({ name, country, address: address || null, website: website || null });
		busy = false;
		if (added) onclose();
	}
</script>

<form onsubmit={submit}>
	<h2>Add a missing restaurant</h2>
	<p class="muted">Stand at the restaurant: it goes on the map where you are.</p>
	<label>Name <input bind:value={name} required maxlength="100" autocomplete="off" /></label>
	<label>
		Food from
		<select bind:value={country} required>
			<option value="" disabled>Choose a country</option>
			{#each countries as c (c.code)}<option value={c.code}>{c.flag} {c.name}</option>{/each}
		</select>
	</label>
	<label>Address <span class="muted">(optional)</span> <input bind:value={address} maxlength="200" autocomplete="street-address" /></label>
	<label>Website <span class="muted">(optional)</span> <input bind:value={website} type="url" maxlength="300" placeholder="https://" /></label>
	<p class="muted small">
		Only you see it until it is on OpenStreetMap. We ask its mappers to add it by posting a note, shared under the
		<a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener">Open Database License</a>.
	</p>
	<div class="actions">
		<button type="button" class="cancel" disabled={busy} onclick={onclose}>Cancel</button>
		<button type="submit" class="add" disabled={busy}>{busy ? 'Adding…' : 'Add it here'}</button>
	</div>
</form>

<style>
	form {
		display: grid;
		gap: 10px;
	}

	label {
		display: grid;
		gap: 4px;
		font-weight: 700;
	}

	label .muted {
		font-weight: 500;
	}

	input,
	select {
		padding: 9px 12px;
		border-radius: 10px;
		border: 1px solid var(--line);
		background: var(--bg);
		color: var(--text);
		font: inherit;
		font-weight: 500;
	}

	.small {
		font-size: 12px;
	}

	.small a {
		color: inherit;
	}

	.actions {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
	}

	.actions button {
		padding: 9px 14px;
		border-radius: 12px;
		font-weight: 800;
	}

	.add {
		background: var(--accent);
		color: var(--on-accent);
	}

	.add:disabled {
		opacity: 0.7;
	}
</style>
