import { raw, write, writeMine } from './client';

export const pushKey = () => raw<{ public_key: string }>('GET', '/api/push/key', null);

export interface PushTarget {
	/** Area id. */
	area: string;
	timezone: string;
}

export const savePushSubscription = (subscription: PushSubscriptionJSON, target: PushTarget) =>
	write<void>('PUT', '/api/push/subscription', { ...subscription, ...target });

export const deletePushSubscription = (endpoint: string) =>
	writeMine('POST', '/api/push/subscription/delete', { endpoint });
