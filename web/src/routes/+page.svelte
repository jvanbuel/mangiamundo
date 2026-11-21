<script lang="ts">
	import { onMount } from 'svelte';
	import { version } from '$app/env';
	import AddPlace from '#lib/AddPlace.svelte';
	import Badges from '#lib/Badges.svelte';
	import CountryDetail from '#lib/CountryDetail.svelte';
	import ComingUp from '#lib/ComingUp.svelte';
	import CountryList from '#lib/CountryList.svelte';
	import LocationPicker from '#lib/LocationPicker.svelte';
	import Passport from '#lib/Passport.svelte';
	import PlaceDetail from '#lib/PlaceDetail.svelte';
	import ProgressWheel from '#lib/ProgressWheel.svelte';
	import Toasts from '#lib/Toasts.svelte';
	import WorldMap from '#lib/WorldMap.svelte';
	import MapPin from '@lucide/svelte/icons/map-pin';
	import Navigation from '@lucide/svelte/icons/navigation';
	import Settings from '@lucide/svelte/icons/settings';
	import { app, init, lookAgainNearby, refresh, select, setProjection, setTheme, toast, type Theme } from '#lib/state.svelte.ts';

	let pickerOpen = $state(false);
	let settingsOpen = $state(false);
	let adding = $state(false);

	const selected = $derived(app.discover?.countries.find((c) => c.code === app.selected) ?? null);
	const earned = $derived(app.discover?.badges.filter((b) => b.earned).length ?? 0);
	const place = $derived(app.location?.area.name ?? null);

	const THEMES: { id: Theme; label: string }[] = [
		{ id: 'system', label: 'Auto' },
		{ id: 'light', label: 'Light' },
		{ id: 'dark', label: 'Dark' }
	];
	let systemDark = $state(false);

	onMount(() => {
		init();
		const media = matchMedia('(prefers-color-scheme: dark)');
		const update = () => (systemDark = media.matches);
		update();
		media.addEventListener('change', update);
		return () => media.removeEventListener('change', update);
	});

	function pick(code: string, name: string, available: boolean) {
		if (available) {
			app.tab = 'countries';
			select(code);
		} else {
			toast(null, `No ${name} food here yet`, `No places from ${name} were found in ${place}.`);
		}
	}
</script>

{#snippet addEntry()}
	{#if adding}
		<div class="card"><AddPlace onclose={() => (adding = false)} /></div>
	{:else}
		<p class="muted add-place">Is a restaurant missing? <button onclick={() => (adding = true)}>Add it</button></p>
		<p class="muted add-place">
			Just added it in an OpenStreetMap app?
			<button disabled={app.lookingNearby} onclick={lookAgainNearby}>{app.lookingNearby ? 'Looking…' : 'Look again'}</button>
		</p>
	{/if}
{/snippet}

{#if app.areaProgress}<ProgressWheel progress={app.areaProgress} />{/if}
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
			<div class="toggle" role="group" aria-label="Map projection">
				<button aria-pressed={app.projection === 'globe'} onclick={() => setProjection('globe')}>Globe</button>
				<button aria-pressed={app.projection === 'mercator'} onclick={() => setProjection('mercator')}>Flat</button>
			</div>
		</div>

		<ul class="legend" aria-label="Legend">
			<li><span style:background="var(--available)"></span>To discover</li>
			<li><span style:background="var(--discovered)"></span>Tasted</li>
			<li><span style:background="var(--completed)"></span>Completed</li>
			<li><span style:background="var(--land)"></span>None nearby</li>
		</ul>
	</div>

	<aside>
		<header>
			<h1>
				<img src="/icon.svg" alt="" width="32" height="32" />Mangiamundo
				<button class="gear" aria-label="Settings" aria-expanded={settingsOpen} onclick={() => (settingsOpen = !settingsOpen)}>
					<Settings size={20} aria-hidden="true" />
				</button>
			</h1>
			{#if !app.location}<p class="tagline">Eat the world, start with your city</p>{/if}
			{#if app.location}
				<button class="where" aria-expanded={pickerOpen} onclick={() => (pickerOpen = !pickerOpen)}>
					{#if app.location.source === 'gps'}<Navigation size={15} strokeWidth={2.5} aria-hidden="true" />{:else}<MapPin size={15} strokeWidth={2.5} aria-hidden="true" />{/if}
					<span class="name">{place}</span>
					{#if app.location.area.kind === 'postal_code'}<span class="muted">· postal code</span>{/if}
					<span class="change">{pickerOpen ? 'Close' : 'Change'}</span>
				</button>
			{/if}
		</header>

		{#if settingsOpen}
			<div class="card settings">
				<h2>Settings</h2>
				<div class="setting">
					<span id="theme-label">Theme</span>
					<div class="toggle" role="group" aria-labelledby="theme-label">
						{#each THEMES as t (t.id)}
							<button aria-pressed={app.theme === t.id} onclick={() => setTheme(t.id)}>{t.label}</button>
						{/each}
					</div>
				</div>
				<p><a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener">Map data © OpenStreetMap contributors</a></p>
				<p class="build">Web {version}{app.server ? ` · Server ${app.server}` : ''}</p>
			</div>
		{/if}

		{#if pickerOpen && app.location}
			<div class="card"><LocationPicker onclose={() => (pickerOpen = false)} /></div>
		{/if}

		{#if !app.location}
			<div class="card welcome">
				<h2>Which of the world's kitchens are near you?</h2>
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
			<p class="muted pad">Scouting restaurants in {place}…</p>
		{:else if app.discover.world.available === 0}
			<div class="card">
				<h2>Nothing found here yet</h2>
				<p class="muted">
					No restaurants with a national cuisine turned up in {place}.
					{app.location.area.kind === 'postal_code' ? 'Try the whole city or' : 'Try'} another location.
				</p>
			</div>
			{@render addEntry()}
		{:else}
			<div class="card" class:dim={app.loading}><Passport discover={app.discover} /></div>

			{#if app.celebrations.length && !selected && !app.placeId}
				<div class="card"><ComingUp celebrations={app.celebrations} /></div>
			{/if}

			{#if app.placeId}
				<section><PlaceDetail place={app.detail} /></section>
			{:else if selected}
				<section><CountryDetail progress={selected} onback={() => select(null)} /></section>
			{:else}
				<div class="tabs" role="tablist">
					<button role="tab" aria-selected={app.tab === 'countries'} onclick={() => (app.tab = 'countries')}>
						Countries <span>{app.discover.world.available}</span>
					</button>
					<button role="tab" aria-selected={app.tab === 'badges'} onclick={() => (app.tab = 'badges')}>
						Badges <span>{earned}/{app.discover.badges.length}</span>
					</button>
				</div>
				{#if app.tab === 'countries'}
					<CountryList discover={app.discover} onselect={select} />
					{@render addEntry()}
				{:else}
					<Badges badges={app.discover.badges} events={app.discover.events} />
				{/if}
			{/if}
		{/if}

	</aside>
</main>

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
		font-size: var(--text-xl);
	}

	h1 {
		display: flex;
		align-items: center;
	}

	.tagline {
		margin: 2px 0 0 42px;
		color: var(--muted);
		font-weight: 700;
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
		font-weight: 700;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.where .change {
		margin-left: auto;
		color: var(--accent);
		font-size: var(--text-sm);
	}

	/* Panels sit on a tinted surface. The heavy lower edge is kept for things you can press. */
	.card {
		padding: 16px;
		border-radius: 14px;
		background: var(--panel-raised);
		transition: opacity 0.2s;
	}

	.card.dim {
		opacity: 0.6;
	}

	.card h2 {
		font-size: var(--text-lg);
		margin-bottom: 6px;
	}

	.card p {
		margin: 0 0 12px;
	}

	.welcome h2 {
		font-size: var(--text-xl);
		line-height: 1.25;
	}

	.gear {
		margin-left: auto;
		display: flex;
		padding: 4px;
		color: var(--muted);
	}

	.settings p.build {
		margin: 0;
		font-size: var(--text-xs);
		color: var(--muted);
	}

	.pad {
		padding: 0 4px;
	}

	.retry {
		padding: 8px 14px;
		border-radius: 10px;
		background: var(--accent);
		color: var(--on-accent);
		font-weight: 700;
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
		font-weight: 700;
	}

	.tabs button[aria-selected='true'] {
		color: var(--text);
		border-bottom-color: var(--accent);
	}

	.tabs span {
		font-weight: 500;
		font-size: var(--text-xs);
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

	.setting {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		margin: 10px 0 14px;
		font-weight: 700;
	}

	.settings .toggle {
		background: var(--panel);
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
		font-size: var(--text-sm);
		font-weight: 700;
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
		font-size: var(--text-xs);
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
		/* Keeps the pale "not in this area" swatch visible on the pill. */
		box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.15);
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
			padding: 6px 10px;
			gap: 2px 10px;
		}

		.controls {
			top: 8px;
			right: 8px;
		}
	}
</style>
