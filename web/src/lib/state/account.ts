import * as api from '../api';
import { app, toast } from './app.svelte';
import { loadPlace } from './details';
import { loadPlaces, refresh } from './discover';

/**
 * The server no longer knows this device: it was signed out from another device, or its
 * account is gone. Says so, and shows everything again as for someone without an account,
 * so that nothing of the old account stays on screen as if it were still here. A new
 * account only starts with the next check-in or other change.
 */
export function signedOut() {
	toast(null, 'You were signed out', 'This device no longer has your progress. Sign in or use a recovery code to get it back.');
	// Whatever shows the account reads it again.
	app.publicLink++;
	void accountChanged();
}

/** Goes up with every change of account, so an answer for the previous one can be told apart. */
let accountGeneration = 0;

/** The account's generation, to compare with one read before an answer came. */
export function currentGeneration() {
	return accountGeneration;
}

/** After this device changes account: the open list and details show the new account's visits. */
export async function accountChanged() {
	accountGeneration++;
	await Promise.all([refresh(), loadPlaces(app.selected), app.placeId && loadPlace(app.placeId), loadAccount()]);
}

/** Loads the account; a failure only means the header can't say whether it's signed in. */
export async function loadAccount() {
	try {
		app.account = await api.account();
	} catch {
		app.account = null;
	}
}
