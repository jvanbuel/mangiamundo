<script lang="ts">
	import MapPin from '@lucide/svelte/icons/map-pin';
	import Navigation from '@lucide/svelte/icons/navigation';
	import Settings from '@lucide/svelte/icons/settings';
	import LogIn from '@lucide/svelte/icons/log-in';
	import { app } from './state.svelte';

	let { place, pickerOpen = $bindable(), settingsOpen = $bindable() }: {
		/** The area's name. */
		place: string | null;
		pickerOpen: boolean;
		settingsOpen: boolean;
	} = $props();

	/** Who the header shows as signed in: a name or address, and a photo when Google gave one. */
	const signedIn = $derived.by(() => {
		const ids = app.account?.identities ?? [];
		if (!ids.length) return null;
		const label =
			ids.find((i) => i.name)?.name ??
			ids.find((i) => i.email)?.email ??
			({ apple: 'Apple', google: 'Google', email: 'Email' } as Record<string, string>)[ids[0].provider] ??
			'Signed in';
		return { label, picture: ids.find((i) => i.picture)?.picture ?? null, initial: label.trim().charAt(0).toUpperCase() };
	});
</script>

<header>
	<h1>
		<img src="/icon.svg" alt="" width="32" height="32" />Mangiamundo
		{#if app.account}
			<!-- Whether this device is signed in, always in view; opens the account settings. -->
			<button
				class="who"
				class:signed={signedIn}
				onclick={() => { settingsOpen = true; pickerOpen = false; }}
				title={signedIn ? `Signed in as ${signedIn.label}` : 'Sign in to keep your progress on all your devices'}
				aria-label={signedIn ? `Signed in as ${signedIn.label}. Account settings` : 'Sign in'}
			>
				{#if signedIn}
					{#if signedIn.picture}
						<img class="avatar" src={signedIn.picture} alt="" width="30" height="30" referrerpolicy="no-referrer" />
					{:else}
						<span class="avatar initial" aria-hidden="true">{signedIn.initial}</span>
					{/if}
				{:else}
					<LogIn size={14} strokeWidth={2.5} aria-hidden="true" /><span class="label">Sign in</span>
				{/if}
			</button>
		{/if}
		<button class="gear" aria-label="Settings" aria-expanded={settingsOpen} onclick={() => { settingsOpen = !settingsOpen; pickerOpen = false; }}>
			<Settings size={20} aria-hidden="true" />
		</button>
	</h1>
	{#if !app.location}<p class="tagline">Eat the world, start with your city</p>{/if}
	{#if app.location}
		<button class="where" aria-expanded={pickerOpen} onclick={() => { pickerOpen = !pickerOpen; settingsOpen = false; }}>
			{#if app.location.source === 'gps'}<Navigation size={15} strokeWidth={2.5} aria-hidden="true" />{:else}<MapPin size={15} strokeWidth={2.5} aria-hidden="true" />{/if}
			<span class="name">{place}</span>
			{#if app.location.area.kind === 'postal_code'}<span class="muted">· postal code</span>{/if}
			<span class="change">{pickerOpen ? 'Close' : 'Change'}</span>
		</button>
	{/if}
</header>

<style>
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

	/* 44px to tap, without pushing the heading around. */
	.who {
		margin-left: auto;
		display: inline-flex;
		align-items: center;
		gap: 5px;
		max-width: 170px;
		padding: 5px 10px;
		border-radius: 999px;
		border: 1px solid var(--line);
		background: var(--panel);
		color: var(--accent-text);
		font-size: var(--text-xs);
		font-weight: 700;
	}

	.who .label {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	/* Signed in: just the round avatar, as most apps show it. */
	.who.signed {
		padding: 2px;
		border: 2px solid var(--accent);
		background: var(--panel);
	}

	.avatar {
		flex: none;
		width: 30px;
		height: 30px;
		border-radius: 50%;
		object-fit: cover;
	}

	.avatar.initial {
		display: grid;
		place-items: center;
		background: var(--accent);
		color: var(--on-accent);
		font-size: 14px;
		font-weight: 800;
	}

	.gear {
		margin: -12px -12px -12px 4px;
		display: flex;
		padding: 12px;
		color: var(--muted);
	}
</style>
