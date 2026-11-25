<script lang="ts">
	import { version } from '$app/env';
	import Account from './Account.svelte';
	import { app, setTheme, type Theme } from './state.svelte';

	const THEMES: { id: Theme; label: string }[] = [
		{ id: 'system', label: 'Auto' },
		{ id: 'light', label: 'Light' },
		{ id: 'dark', label: 'Dark' }
	];
</script>

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
	<Account />
	<p class="build">
		<a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener">Map data © OpenStreetMap contributors</a> ·
		<a href="/privacy/">Privacy</a> · Web {version}{app.server ? ` · Server ${app.server}` : ''}
	</p>
</div>

<style>
	/* Panels sit on a tinted surface. The heavy lower edge is kept for things you can press. */
	.card {
		padding: 16px;
		border-radius: 14px;
		background: var(--panel-raised);
		transition: opacity 0.2s;
	}

	.card h2 {
		font-size: var(--text-lg);
		margin-bottom: 6px;
	}

	.settings p.build {
		margin: 0;
		padding-top: 14px;
		border-top: 1px solid var(--line);
		font-size: var(--text-xs);
		line-height: 1.6;
		color: var(--muted);
	}

	.settings p.build a {
		color: inherit;
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
</style>
