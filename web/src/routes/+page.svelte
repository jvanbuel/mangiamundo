<script lang="ts">
	import { onMount } from 'svelte';
	import Badges from '#lib/Badges.svelte';
	import CountryDetail from '#lib/CountryDetail.svelte';
	import CountryList from '#lib/CountryList.svelte';
	import LocationPicker from '#lib/LocationPicker.svelte';
	import Passport from '#lib/Passport.svelte';
	import Toasts from '#lib/Toasts.svelte';
	import WorldMap from '#lib/WorldMap.svelte';
	import { app, init, refresh, select, setProjection, setTheme, toast, type Theme } from '#lib/state.svelte.ts';

	let tab = $state<'countries' | 'badges'>('countries');
	let pickerOpen = $state(false);

	const selected = $derived(app.discover?.countries.find((c) => c.code === app.selected) ?? null);
	const earned = $derived(app.discover?.badges.filter((b) => b.earned).length ?? 0);
	const place = $derived(app.location?.name ?? (app.location ? 'your location' : null));

	const THEMES: { id: Theme; label: string }[] = [
		{ id: 'system', label: 'Auto' },
		{ id: 'light', label: 'Light' },
		{ id: 'dark', label: 'Dark' }
	];
	const theme = $derived(THEMES.find((t) => t.id === app.theme) ?? THEMES[0]);
	let systemDark = $state(false);

	onMount(() => {
		init();
		const media = matchMedia('(prefers-color-scheme: dark)');
		const update = () => (systemDark = media.matches);
		update();
		media.addEventListener('change', update);
		return () => media.removeEventListener('change', update);
	});

	function nextTheme() {
		setTheme(THEMES[(THEMES.indexOf(theme) + 1) % THEMES.length].id);
	}

	function pick(code: string, name: string, available: boolean) {
		if (available) {
			tab = 'countries';
			select(code);
		} else {
			toast('🔍', `No ${name} here yet`, `No places from ${name} were found within ${app.radiusKm} km.`);
		}
	}
</script>

<Toasts />

<main>
	<div class="stage">
		{#if app.catalog}
			<WorldMap
				countries={app.catalog.countries}
				progress={app.discover?.countries ?? []}
				selected={app.selected}
				location={app.location}
				projection={app.projection}
				theme={`${app.theme}-${systemDark}`}
				onselect={pick}
			/>
		{/if}

		<div class="controls">
			<button class="theme" onclick={nextTheme} aria-label="Colour theme: {theme.label}. Click to change.">
				<span class="muted">Theme</span>
				{theme.label}
			</button>
			<div class="toggle" role="group" aria-label="Map projection">
				<button aria-pressed={app.projection === 'globe'} onclick={() => setProjection('globe')}>Globe</button>
				<button aria-pressed={app.projection === 'mercator'} onclick={() => setProjection('mercator')}>Flat</button>
			</div>
		</div>

		<ul class="legend" aria-label="Legend">
			<li><span style:background="var(--available)"></span>To discover</li>
			<li><span style:background="var(--discovered)"></span>Tasted</li>
			<li><span style:background="var(--completed)"></span>Completed</li>
			<li><span style:background="var(--land)"></span>Not in this city</li>
		</ul>
	</div>

	<aside>
		<header>
			<h1><img src="/icon.svg" alt="" width="32" height="32" />Mangiamundo</h1>
			{#if app.location}
				<button class="where" aria-expanded={pickerOpen} onclick={() => (pickerOpen = !pickerOpen)}>
					<span aria-hidden="true">{app.location.source === 'gps' ? '◎' : '⌖'}</span>
					<span class="name">{place}</span>
					<span class="muted">· {app.radiusKm} km</span>
					<span class="change">{pickerOpen ? 'Close' : 'Change'}</span>
				</button>
			{/if}
		</header>

		{#if pickerOpen && app.location}
			<div class="card"><LocationPicker onclose={() => (pickerOpen = false)} /></div>
		{/if}

		{#if !app.location}
			<div class="card welcome">
				<h2>Eat your way around the world without leaving town.</h2>
				<p class="muted">
					Tell us where you are and we will light up every country whose kitchen you can visit nearby.
				</p>
				<LocationPicker />
			</div>
		{:else if app.error}
			<div class="card">
				<p>{app.error}</p>
				<button class="retry" onclick={refresh}>Try again</button>
			</div>
		{:else if !app.discover}
			<p class="muted pad">Scouting restaurants around {place}…</p>
		{:else if app.discover.world.available === 0}
			<div class="card">
				<h2>Nothing found here yet</h2>
				<p class="muted">
					No restaurants with a national cuisine turned up within {app.radiusKm} km. Try a larger radius or
					another location.
				</p>
			</div>
		{:else}
			<div class="card" class:dim={app.loading}><Passport discover={app.discover} /></div>

			{#if selected}
				<div class="card"><CountryDetail progress={selected} onback={() => select(null)} /></div>
			{:else}
				<div class="tabs" role="tablist">
					<button role="tab" aria-selected={tab === 'countries'} onclick={() => (tab = 'countries')}>
						Countries <span>{app.discover.world.available}</span>
					</button>
					<button role="tab" aria-selected={tab === 'badges'} onclick={() => (tab = 'badges')}>
						Badges <span>{earned}/{app.discover.badges.length}</span>
					</button>
				</div>
				{#if tab === 'countries'}
					<CountryList discover={app.discover} onselect={select} />
				{:else}
					<Badges badges={app.discover.badges} />
				{/if}
			{/if}
		{/if}
	</aside>
</main>

<style>
	main {
		display: grid;
		grid-template-columns: 400px 1fr;
		height: 100dvh;
	}

	.stage {
		position: relative;
		grid-column: 2;
		grid-row: 1;
		min-height: 0;
		overflow: hidden;
	}

	aside {
		grid-column: 1;
		grid-row: 1;
		overflow-y: auto;
		padding: 20px 16px 32px;
		background: var(--panel);
		border-right: 1px solid var(--line);
		display: flex;
		flex-direction: column;
		gap: 14px;
	}

	h1 {
		font-size: 22px;
	}

	h1 {
		display: flex;
		align-items: center;
	}

	h1 img {
		margin-right: 10px;
		border-radius: 9px;
	}

	.where {
		display: flex;
		align-items: center;
		gap: 6px;
		width: 100%;
		margin-top: 10px;
		padding: 9px 12px;
		border-radius: 10px;
		border: 1px solid var(--line);
		text-align: left;
	}

	.where .name {
		font-weight: 600;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.where .change {
		margin-left: auto;
		color: var(--accent);
		font-size: 13px;
	}

	.card {
		padding: 16px;
		border-radius: var(--radius);
		border: 1px solid var(--line);
		background: color-mix(in srgb, var(--panel-raised) 45%, var(--panel));
		transition: opacity 0.2s;
	}

	.card.dim {
		opacity: 0.6;
	}

	.card h2 {
		font-size: 18px;
		margin-bottom: 6px;
	}

	.card p {
		margin: 0 0 12px;
	}

	.welcome h2 {
		font-size: 21px;
		line-height: 1.25;
	}

	.pad {
		padding: 0 4px;
	}

	.retry {
		padding: 8px 14px;
		border-radius: 10px;
		background: var(--accent);
		color: var(--on-accent);
		font-weight: 600;
	}

	.tabs {
		display: grid;
		grid-template-columns: 1fr 1fr;
		border-bottom: 1px solid var(--line);
	}

	.tabs button {
		padding: 10px;
		color: var(--muted);
		border-bottom: 2px solid transparent;
		margin-bottom: -1px;
		font-weight: 600;
	}

	.tabs button[aria-selected='true'] {
		color: var(--text);
		border-bottom-color: var(--accent);
	}

	.tabs span {
		font-weight: 400;
		font-size: 12px;
		color: var(--muted);
		margin-left: 4px;
	}

	.controls {
		position: absolute;
		top: 16px;
		right: 16px;
		z-index: 1;
		display: flex;
		gap: 8px;
	}

	.theme {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 0 14px;
		border-radius: 999px;
		background: var(--glass);
		border: 1px solid var(--line);
		font-size: 13px;
		font-weight: 600;
	}

	.theme span {
		font-weight: 400;
	}

	.toggle {
		display: flex;
		padding: 3px;
		border-radius: 999px;
		background: var(--glass);
		border: 1px solid var(--line);
	}

	.toggle button {
		padding: 6px 16px;
		border-radius: 999px;
		font-size: 13px;
		font-weight: 600;
		color: var(--muted);
	}

	.toggle button[aria-pressed='true'] {
		background: var(--accent);
		color: var(--on-accent);
	}

	.legend {
		position: absolute;
		left: 16px;
		bottom: 16px;
		z-index: 1;
		list-style: none;
		margin: 0;
		padding: 8px 12px;
		display: flex;
		flex-wrap: wrap;
		gap: 4px 14px;
		border-radius: 10px;
		background: var(--glass);
		border: 1px solid var(--line);
		font-size: 12px;
		color: var(--muted);
	}

	.legend li {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.legend span {
		width: 10px;
		height: 10px;
		border-radius: 3px;
	}

	@media (max-width: 760px) {
		main {
			grid-template-columns: 1fr;
			grid-template-rows: 42dvh 1fr;
		}

		.stage {
			grid-column: 1;
			grid-row: 1;
		}

		aside {
			grid-column: 1;
			grid-row: 2;
			border-right: 0;
			border-top: 1px solid var(--line);
			padding: 14px 16px 28px;
		}

		.legend {
			left: 8px;
			bottom: 8px;
			right: 44px;
			gap: 2px 10px;
			font-size: 11px;
		}

		.controls {
			top: 8px;
			right: 8px;
		}
	}
</style>
