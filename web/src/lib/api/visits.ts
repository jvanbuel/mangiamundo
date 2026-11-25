import type { ShareLink, VisitRemoved, VisitResult } from '../types';
import { area, tz, write, writeMine } from './client';

/** Checks in, and answers with the progress in the area afterwards and what the check-in unlocked. */
export const checkIn = (placeId: string, areaId: string, here: { lat: number; lng: number } | null) =>
	write<VisitResult>('POST', `/api/places/${encodeURIComponent(placeId)}/visits?${area(areaId)}&${tz()}`, {
		position: here
	});

/** Takes a check-in back. The answer has the visits left at the place and the progress in the area. */
export const removeVisit = (visitId: number, areaId: string) =>
	write<VisitRemoved>('DELETE', `/api/visits/${visitId}?${area(areaId)}&${tz()}`);

/** Makes (or moves to this area) the public page of the user's progress. */
export const shareProgress = (areaId: string) => write<ShareLink>('POST', `/api/share?${area(areaId)}`);

/** Turns the public page off. */
export const unshareProgress = () => writeMine('DELETE', '/api/share');

/** A public page for one check-in. */
export const shareVisit = (visitId: number) => write<ShareLink>('POST', `/api/visits/${visitId}/share`);
