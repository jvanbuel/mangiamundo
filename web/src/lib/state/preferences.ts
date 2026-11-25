import { app, type Theme } from './app.svelte';

export const PROJECTION_KEY = 'mangiamundo.projection';
export const THEME_KEY = 'mangiamundo.theme';

export function setProjection(projection: 'globe' | 'mercator') {
	app.projection = projection;
	localStorage.setItem(PROJECTION_KEY, JSON.stringify(projection));
}

export function setTheme(theme: Theme) {
	// The attribute goes first so anything reacting to `app.theme` already sees the new colours.
	if (theme === 'system') delete document.documentElement.dataset.theme;
	else document.documentElement.dataset.theme = theme;
	localStorage.setItem(THEME_KEY, JSON.stringify(theme));
	app.theme = theme;
}
