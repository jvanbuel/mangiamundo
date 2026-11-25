import * as push from '../push';
import { app, message, toast } from './app.svelte';

export async function checkReminders() {
	if (push.needsInstallForPush()) app.reminders = 'install';
	else if (!push.pushSupported()) app.reminders = 'unsupported';
	else app.reminders = (await push.currentSubscription()) && Notification.permission === 'granted' ? 'on' : 'off';
}

export function pushTarget() {
	return { area: app.location!.area.id };
}

export async function toggleReminders() {
	if (!app.location || app.remindersBusy) return;
	app.remindersBusy = true;
	try {
		if (app.reminders === 'on') {
			await push.disableReminders();
			app.reminders = 'off';
		} else {
			await push.enableReminders(pushTarget());
			app.reminders = 'on';
			toast(null, 'Reminders on', 'We will let you know when a country near you has something to celebrate.');
		}
	} catch (error) {
		toast(null, 'Could not change reminders', message(error));
	} finally {
		app.remindersBusy = false;
	}
}
