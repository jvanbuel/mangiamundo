<script lang="ts">
	import { onMount } from 'svelte';
	import AddEntry from '#lib/AddEntry.svelte';
	import Badges from '#lib/Badges.svelte';
	import CountryDetail from '#lib/CountryDetail.svelte';
	import ComingUp from '#lib/ComingUp.svelte';
	import CountryList from '#lib/CountryList.svelte';
	import LocationPicker from '#lib/LocationPicker.svelte';
	import MapStage from '#lib/MapStage.svelte';
	import PanelHeader from '#lib/PanelHeader.svelte';
	import Passport from '#lib/Passport.svelte';
	import PlaceDetail from '#lib/PlaceDetail.svelte';
	import ProgressWheel from '#lib/ProgressWheel.svelte';
	import SettingsCard from '#lib/SettingsCard.svelte';
	import Toasts from '#lib/Toasts.svelte';
	import UnknownFood from '#lib/UnknownFood.svelte';
	import { app, init, refresh, select, toast } from '#lib/state.svelte.ts';
	let pickerOpen = $state(false);
	let settingsOpen = $state(false);
	let adding = $state(false);

	const selected = $derived(app.discover?.countries.find((c) => c.code === app.selected) ?? null);
	const earned = $derived(app.discover?.badges.filter((b) => b.earned).length ?? 0);
	const place = $derived(app.location?.area.name ?? null);

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

{#if app.areaProgress}<ProgressWheel progress={app.areaProgress} />{/if}
<Toasts />

<main>
	<MapStage {systemDark} onselect={pick} />

	<aside>
		<PanelHeader {place} bind:pickerOpen bind:settingsOpen />

		{#if settingsOpen}
			<SettingsCard />
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
				<p class="muted about">
					Check in at restaurants to taste each country's food and earn levels and badges. No account
					needed: your progress stays on this device. Signing in with Google, Apple or email is optional
					and only keeps your progress on all your devices. <a href="/privacy/">Privacy policy</a>
				</p>
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
			<AddEntry bind:adding />
		{:else}
			<!-- Progress is for the overview; a country or place gets the room to itself. -->
			{#if !selected && !app.placeId}
				<div class="card" class:dim={app.loading}><Passport discover={app.discover} areaId={app.location.area.id} /></div>
			{/if}

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
					{#if !adding}<UnknownFood areaId={app.location.area.id} />{/if}
					<AddEntry bind:adding />
				{:else}
					<Badges badges={app.discover.badges} events={app.discover.events} quests={app.discover.quests} />
				{/if}
			{/if}
		{/if}

		<footer class="site-footer"><a href="/privacy/">Privacy</a> · <a href="mailto:hello@mangiamundo.com">Contact</a></footer>
	</aside>
</main>

<style>
	main {
		display: grid;
		grid-template-columns: 400px 1fr;
		height: 100dvh;
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

	.about {
		margin: 12px 0 0;
		font-size: var(--text-xs);
	}

	/* Always at the bottom of the panel, however short its content. */
	.site-footer {
		margin-top: auto;
		padding-top: 8px;
		font-size: var(--text-xs);
		color: var(--muted);
	}

	.site-footer a {
		color: inherit;
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

	@media (max-width: 760px) {
		main {
			grid-template-columns: 1fr;
			grid-template-rows: 34dvh 1fr;
		}

		aside {
			grid-column: 1;
			grid-row: 2;
			border-right: 0;
			border-top: 1px solid var(--line);
			padding: 14px 16px 28px;
		}
	}
</style>
