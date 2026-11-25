import type { Account, Area, AreaProgress, BadgeKind, Catalog, Celebration, Country, Discover, Location, Place } from '../types';

export type Theme = 'system' | 'light' | 'dark';

/** What a toast shows on its left: a flag (or other text), a badge medal, or nothing. */
export type ToastIcon = { text: string } | { badge: BadgeKind | 'event' } | null;

export interface Toast {
	id: number;
	icon: ToastIcon;
	title: string;
	text: string;
}

export function load<T>(key: string): T | null {
	try {
		return JSON.parse(localStorage.getItem(key) ?? 'null');
	} catch {
		return null;
	}
}

export const app = $state({
	/** The server's version and commit; null until it answers, 'not reached' after a failure. */
	server: null as string | null,
	catalog: null as Catalog | null,
	location: null as Location | null,
	discover: null as Discover | null,
	loading: false,
	/** How far the area's restaurants have loaded, while that takes a while. */
	areaProgress: null as AreaProgress | null,
	error: null as string | null,
	/** Code of the country whose places are shown. */
	selected: null as string | null,
	places: [] as Place[],
	placesLoading: false,
	/** Id of the place whose details are open, and the details once loaded. */
	placeId: null as string | null,
	detail: null as Place | null,
	/** The last place whose details closed, so its link in the list gets focus back. */
	closedPlace: null as string | null,
	/** The place whose check-in or undo is running. One at a time, across cards and details. */
	busyPlace: null as string | null,
	/** The place being read again from OpenStreetMap. */
	refreshingPlace: null as string | null,
	/** While true the map turns the globe; when it ends the map flies to the selected country. */
	spinning: false,
	/** The place the last spin landed on, whose details offer to spin again. */
	spun: null as string | null,
	/** Asking OpenStreetMap for places just mapped nearby. */
	lookingNearby: false,
	locating: false,
	locateError: null as string | null,
	projection: 'globe' as 'globe' | 'mercator',
	theme: 'system' as Theme,
	tab: 'countries' as 'countries' | 'badges',
	celebrations: [] as Celebration[],
	/** Web Push reminders: on, off, not possible here, or possible once installed (iPhone). */
	reminders: 'off' as 'on' | 'off' | 'unsupported' | 'install',
	remindersBusy: false,
	toasts: [] as Toast[],
	/** This device's account, for showing whether and how it is signed in. */
	account: null as Account | null,
	/** Goes up whenever the public link is made or turned off, or the device is signed out, for whatever shows the account. */
	publicLink: 0,
	/** Areas used lately, newest first, for switching back quickly. */
	recents: [] as Area[]
});

let toastId = 0;

export function country(code: string): Country | undefined {
	return app.catalog?.countries.find((c) => c.code === code);
}

export function toast(icon: ToastIcon, title: string, text: string) {
	const id = ++toastId;
	app.toasts.push({ id, icon, title, text });
	setTimeout(() => {
		app.toasts = app.toasts.filter((t) => t.id !== id);
	}, 5000);
}

export function message(error: unknown): string {
	return error instanceof Error ? error.message : 'Something went wrong.';
}
